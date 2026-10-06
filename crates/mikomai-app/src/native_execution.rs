//! Rust-owned OS and device execution, including immutable approval and credentials.
use serde_json::{json,Value};
use std::{ffi::{CStr,CString,c_char,c_void}};
use tokio::sync::watch;
use mikomai_adapters::device_worker::{WorkerPool,WorkerResult};
use mikomai_core::{OperationStatus,domain::OperationGate};

pub fn connection(target:&str)->Result<Value,String> {
    let records=crate::shared_service().load_document("connections")?.unwrap_or(json!([]));
    records.as_array().ok_or("invalid connections schema")?.iter().find(|v|["id","name","host"].iter().any(|k|v[*k].as_str().is_some_and(|s|s.eq_ignore_ascii_case(target))))
        .cloned().ok_or_else(||"registered device not found".into())
}
pub fn snapshot(connection:&Value)->Result<Value,String> {
    let mut value=connection.clone(); let id=value["id"].as_str().ok_or("credential reference missing")?;
    value["credentialsFingerprint"]=json!(mikomai_adapters::secrets::fingerprint(id)?);
    value["connectionType"]=json!(connection["connectionType"].as_str().unwrap_or("SSH"));
    value["port"]=json!(connection["port"].as_str().unwrap_or("")); Ok(value)
}
fn pool()->Result<&'static WorkerPool,String> {
    crate::shared_service().device_workers.get_or_init(|| mikomai_adapters::device_worker::bundled_path().map(|p|WorkerPool::new(&p))).as_ref().map_err(Clone::clone)
}
pub async fn read_task(task_id:&str,connection:&Value,commands:Vec<String>,timeout:u32,cancel:&mut watch::Receiver<bool>,scheduler:&crate::scheduling::TaskScheduler)->Result<WorkerResult,String> {
    let transport=connection["connectionType"].as_str().unwrap_or("SSH");
    let serial=transport.eq_ignore_ascii_case("console")||transport.eq_ignore_ascii_case("serial");
    let key=if serial {format!("serial:{}",connection["host"].as_str().unwrap_or(""))} else {format!("device:{}",uuid::Uuid::parse_str(connection["id"].as_str().ok_or("device ID missing")?).map_err(|_|"invalid device ID")?)};
    let _lease=crate::api::engine().wait_device(task_id,&key,false,serial,cancel).await?;
    let _permit=scheduler.acquire(cancel).await?;
    device_locked(connection,"show",json!(commands),timeout as f64,cancel.clone()).await
}
pub async fn device(connection:&Value,op:&str,commands:Value,timeout:f64,watch_run:bool,cancel:&mut watch::Receiver<bool>)->Result<WorkerResult,String> {
    let transport=match connection["connectionType"].as_str().unwrap_or("SSH").to_ascii_lowercase().as_str() {"console"|"serial"=>"serial","telnet"=>"telnet",_=>"ssh"};
    let id=connection["id"].as_str().ok_or("registered device ID missing")?;
    let key=if transport=="serial" {format!("serial:{}",connection["host"].as_str().unwrap_or(""))} else {format!("device:{}",uuid::Uuid::parse_str(id).map_err(|_|"invalid device ID")?)};
    let change=matches!(op,"config"|"console");
    let locks=crate::shared_service().device_locks.get_or_init(Default::default);
    let _lease=if !watch_run {
        if let Some(task_id)=crate::api::current_task_id(){crate::api::engine().wait_device(&task_id,&key,change,transport=="serial",cancel).await?}
        else {locks.acquire(&key,change,transport=="serial",false,crate::portable_app_data_dir()?.join("locks"),cancel).await?}
    } else {
        match locks.acquire(&key,change,transport=="serial",true,crate::portable_app_data_dir()?.join("locks"),cancel).await {
            Ok(lease)=>lease,Err(error)=>{crate::api::lock_audit(&key,"watch_skipped_busy",0)?;return Err(error);}
        }
    };
    device_locked(connection,op,commands,timeout,cancel.clone()).await
}
async fn device_locked(connection:&Value,op:&str,commands:Value,timeout:f64,cancel:watch::Receiver<bool>)->Result<WorkerResult,String> {
    if op=="show" { validate_show(&commands)?; }
    let id=connection["id"].as_str().ok_or("credential reference missing")?;
    let credentials=mikomai_adapters::secrets::load(id)?;
    let secrets=credentials.as_object().unwrap().values().filter_map(Value::as_str).map(str::to_owned).collect::<Vec<_>>();
    let transport=match connection["connectionType"].as_str().unwrap_or("SSH").to_ascii_lowercase().as_str() {"console"|"serial"=>"serial","telnet"=>"telnet",_=>"ssh"};
    let request=json!({"op":op,"transport":transport,"device_type":crate::native_features::canonical_device_id(connection["deviceType"].as_str().unwrap_or("cisco_ios")),"host":connection["host"],"username":connection["username"],"port":connection["port"].as_str().and_then(|s|s.parse::<u16>().ok()).unwrap_or(if transport=="telnet" {23} else {22}),"commands":commands,"credentials":credentials,"timeout":timeout});
    pool()?.next().execute(request,&secrets,cancel).await
}
pub fn execute_tool(tool:&str,target:&Value,args:&Value)->Result<String,String> {
    let (_send,default)=watch::channel(false);
    let mut cancel=crate::scheduling::current_cancellation().unwrap_or(default);
    if matches!(tool,"validate_cisco_config"|"convert_cisco_config"|"self_network_nwdiag") {
        let mut request=args.clone(); request["op"]=json!(match tool {"validate_cisco_config"=>"config_validate","convert_cisco_config"=>"config_convert",_=>"nwdiag_render"});request["timeout"]=json!(30);
        let result=crate::shared_service().run(pool()?.next().execute(request,&[],cancel))??;
        if result.status!="completed" {return Err(result.payload.to_string());}
        if let Some(svg)=result.payload["svg"].as_str() {
            use base64::Engine;
            return Ok(format!("__PORTABLE_ARTIFACT__![Network Diagram](data:image/svg+xml;base64,{})",base64::engine::general_purpose::STANDARD.encode(svg)));
        }
        return Ok(result.payload.to_string());
    }
    if tool.starts_with("self_network_") || matches!(tool,"network_get_ip_info"|"network_list_serial_ports") {
        return host_tool(tool,args);
    }
    let name=target["id"].as_str().or_else(||target["hostname"].as_str()).ok_or("target missing")?;
    let connection=connection(name)?;
    let resource=args["resource"].as_str().unwrap_or("");
    let command=match tool {
        "network_show"=>args["command"].as_str().ok_or("show command missing")?.to_owned(),
        "fetch_config"=>show_config(&connection).into(),"fetch_routing"=>"show ip route".into(),"fetch_arp"=>arp_command(&connection).into(),
        "get_state"=>resource_command(resource,&connection,args)?,_=>return Err("unsupported read-only tool".into())
    };
    let result=crate::shared_service().run(device(&connection,"show",json!([command]),60.,args["watch_run"].as_bool().unwrap_or(false),&mut cancel))??;
    if result.status!="completed" {return Err(result.payload.to_string());}
    let output=result.payload["output"].as_str().unwrap_or("").to_owned();
    if resource=="cpu" {return cpu_usage(&output).map(|v|json!({"usage":v}).to_string());}
    Ok(output)
}
fn show_config(connection:&Value)->&str {match connection["deviceType"].as_str().unwrap_or("") {"juniper_junos"=>"show configuration","yamaha"=>"show config",_=>"show running-config"}}
fn arp_command(connection:&Value)->&str {let kind=connection["deviceType"].as_str().unwrap_or("");if kind.contains("juniper")||kind.contains("yamaha") {"show arp"} else {"show ip arp"}}
fn cpu_command(connection:&Value)->&str {let kind=connection["deviceType"].as_str().unwrap_or("");if kind.contains("juniper") {"show system processes extensive | match CPU"} else if kind.contains("arista") {"show processes top once"} else if kind.contains("yamaha") {"show status cpu"} else if kind.contains("furukawa")||kind.contains("fitel") {"show cpu"} else {"show processes cpu"}}
fn resource_command(resource:&str,connection:&Value,args:&Value)->Result<String,String> {
    Ok(match resource {
        "config"=>show_config(connection),"routes"=>"show ip route","arp"=>arp_command(connection),"ndp"=>"show ipv6 neighbors","interfaces"=>"show interfaces","lldp"=>"show lldp neighbors detail","mac_table"|"mac_entry"=>"show mac address-table","system"=>"show version","bgp"=>"show ip bgp summary","ospf"=>"show ip ospf","ospf_neighbor"=>"show ip ospf neighbor detail","cpu"=>cpu_command(connection),
        "isis"=>"show isis neighbors detail","bfd"=>"show bfd neighbors details","vrrp"=>"show vrrp","lacp"=>"show lacp neighbor","tunnel"=>"show interfaces tunnel","routing_policy"=>"show route-map","prefix_set"=>"show ip prefix-list","policy_forwarding"=>"show ip policy","acl_entry"=>"show access-lists","acl_binding"=>"show ip interface","nat"=>"show ip nat translations verbose","dhcp_relay"=>"show running-config | section interface","qos"=>"show policy-map","qos_interface"=>"show policy-map interface","pim"=>"show ip pim interface","igmp"=>"show ip igmp groups detail","mpls"=>"show mpls forwarding-table","dns_server"=>"show hosts","syslog_server"=>"show logging","aaa_server"=>"show aaa servers","snmp"=>"show snmp","telemetry_subscription"=>"show telemetry ietf subscription all","platform_component"=>"show inventory","ipsec_connection"=>"show crypto ipsec sa","ike_sa"=>"show crypto ikev2 sa detail",
        _=>return Err(format!("unsupported device resource: {resource}"))
    }.to_owned()+if resource=="interfaces" {args["interface"].as_str().filter(|s|!s.is_empty()&&s.chars().all(|c|c.is_ascii_alphanumeric()||"/.-:".contains(c))).map(|s|format!(" {s}")).unwrap_or_default()} else {String::new()}.as_str())
}
fn cpu_usage(text:&str)->Result<f64,String> {
    for pattern in [r"(?i)cpu\s+utilization[^\n:]*:\s*(\d+(?:\.\d+)?)\s*%",r"(?i)cpu[^\n]*?\b(\d+(?:\.\d+)?)\s*(?:%|percent)"] {
        let re=regex::Regex::new(pattern).map_err(|e|e.to_string())?;
        if let Some(value)=re.captures(text).and_then(|c|c.get(1)).and_then(|n|n.as_str().parse::<f64>().ok()).filter(|n|n.is_finite()&&(0.0..=100.0).contains(n)) {return Ok(value);}
    } Err("CPU usage was not present in device output".into())
}
fn validate_show(commands:&Value)->Result<(),String> {
    let commands=commands.as_array().filter(|v|!v.is_empty()).ok_or("show commands missing")?;
    for value in commands {
        let command=value.as_str().ok_or("show command invalid")?;
        if !command.trim().to_ascii_lowercase().starts_with("show ") || command.len()>2048 || command.chars().any(|c|c.is_control()||";&<>`$\\".contains(c)) || command.split('|').skip(1).any(|filter|!matches!(filter.trim().split_whitespace().next(),Some("include"|"exclude"|"begin"|"section"|"match"|"count"))) {return Err("only a single bounded read-only show command is allowed".into());}
    } Ok(())
}

fn host_tool(tool:&str,args:&Value)->Result<String,String> {
    let host=args["host"].as_str().unwrap_or("");
    if tool=="self_network_route" && !["default","destination","table"].contains(&args["scope"].as_str().unwrap_or("default")) {return Err("invalid route scope".into());}
    if matches!(tool,"self_network_ping"|"self_network_traceroute") && (host.is_empty()||host.starts_with('-')||!host.chars().all(|c|c.is_ascii_alphanumeric()||"._:%-".contains(c))) {return Err("invalid host".into());}
    let (path,arguments):(&str,Vec<String>)=match tool {
        "self_network_ping"=>("/sbin/ping",vec!["-c".into(),args["count"].as_u64().unwrap_or(4).clamp(1,20).to_string(),host.into()]),
        "self_network_traceroute"=>("/usr/sbin/traceroute",vec!["-w".into(),"2".into(),"-m".into(),"15".into(),host.into()]),
        "self_network_route"=>if let Some(destination)=args["destination"].as_str() {if destination.starts_with('-')||!destination.chars().all(|c|c.is_ascii_alphanumeric()||".:/%-".contains(c)) {return Err("invalid destination".into());}("/sbin/route",vec!["-n".into(),"get".into(),destination.into()])} else if args["scope"]=="table" {("/usr/sbin/netstat",vec!["-rn".into()])} else {("/sbin/route",vec!["-n".into(),"get".into(),"default".into()])},
        "self_network_ndp"=>("/usr/sbin/ndp",vec!["-a".into()]),
        "network_get_ip_info"=>("/sbin/ifconfig",vec!["-a".into()]),
        "network_list_serial_ports"=>return Ok(serialport_names()),
        "self_network_test_connection"|"self_network_test_net_connection"=>return crate::test_tcp_connection_core(host,args["port"].as_u64().and_then(|v|u16::try_from(v).ok()).filter(|p|*p>0).ok_or("invalid port")?,3000),
        _=>return Err("unsupported host tool".into())
    };
    let output=std::process::Command::new(path).args(arguments).output().map_err(|e|e.to_string())?;
    if !output.status.success() {return Err(String::from_utf8_lossy(&output.stderr).into());}
    Ok(String::from_utf8_lossy(&output.stdout).into())
}
fn serialport_names()->String {std::fs::read_dir("/dev").into_iter().flatten().filter_map(Result::ok).filter(|p|p.file_name().to_string_lossy().starts_with("cu.")).map(|p|p.path().to_string_lossy().into_owned()).collect::<Vec<_>>().join("\n")}

pub fn execute_approved(id:&str,hash:&str)->Result<String,String> {
    let (_sender,cancel)=watch::channel(false);
    execute_approved_with_cancel(id,hash,cancel)
}
pub fn execute_approved_with_cancel(id:&str,hash:&str,mut cancel:watch::Receiver<bool>)->Result<String,String> {
    if *cancel.borrow() {return Err("cancelled".into());}
    let plan={crate::operation_plans()?.lock().map_err(|_|"operation state unavailable")?.get(id).cloned().ok_or("operation plan not found")?};
    if plan.status!=OperationStatus::Approved {return Err("operation requires an unclaimed approval".into());}
    // Exact hash authorization happens before acquiring the device or changing status.
    if plan.plan_hash!=hash {return Err("approval hash mismatch".into());}
    let approved=&plan.args["deviceSnapshot"];
    let connection=connection(approved["id"].as_str().ok_or("device ID missing")?)?;
    let current=snapshot(&connection)?;
    for key in ["id","name","host","username","deviceType","connectionType","port","credentialsFingerprint"] {if current[key]!=approved[key] {return Err("device metadata or credentials changed; create a new plan".into());}}
    let connection_type=connection["connectionType"].as_str().unwrap_or("SSH");
    let serial=connection_type.eq_ignore_ascii_case("console")||connection_type.eq_ignore_ascii_case("serial");
    let key=if serial {format!("serial:{}",connection["host"].as_str().unwrap_or(""))} else {format!("device:{}",uuid::Uuid::parse_str(connection["id"].as_str().unwrap()).map_err(|_|"invalid device ID")?)};
    let locks=crate::shared_service().device_locks.get_or_init(Default::default);
    let _lease=if let Some(task_id)=crate::api::current_task_id(){crate::shared_service().run(crate::api::engine().wait_device(&task_id,&key,true,serial,&mut cancel))??}
    else {crate::shared_service().run(locks.acquire(&key,true,serial,false,crate::portable_app_data_dir()?.join("locks"),&mut cancel))??};
    let _permit=crate::shared_service().run(crate::api::engine().scheduler.acquire(&mut cancel))??;
    if plan.tool_id=="network_config" {
        for command in plan.args["commands"].as_array().ok_or("commands missing")? {crate::validate_native_config_command(command.as_str().ok_or("invalid command")?)?;}
    }
    {
        let mut plans=crate::operation_plans()?.lock().map_err(|_|"operation state unavailable")?;
        let stored=plans.get_mut(id).ok_or("plan missing")?;
        let previous=stored.clone(); OperationGate::begin_execution(stored,hash)?;
        if let Err(error)=crate::persist_operation_plans(&plans) {plans.insert(id.into(),previous);return Err(error);}
        crate::operation_audit_log()?.append(&mikomai_core::audit::record(plan.tool_id.clone(),plan.target.clone(),plan.operation_class,"started",&json!({"plan_hash":hash})))?;
    }
    let outcome=if plan.tool_id=="network_config" {
        for command in plan.args["commands"].as_array().ok_or("commands missing")? {crate::validate_native_config_command(command.as_str().ok_or("invalid command")?)?;}
        crate::shared_service().run(device_locked(&connection,"config",plan.args["commands"].clone(),120.,cancel))?
    } else {
        let secrets=mikomai_adapters::secrets::load(connection["id"].as_str().unwrap())?;
        crate::shared_service().run(crate::execute_approved_operation(id.into(),hash.into(),secrets.to_string()))?.map(|output|WorkerResult{status:"completed".into(),payload:json!({"output":output})})
    };
    let status=match &outcome {Ok(v) if v.status=="completed"=>OperationStatus::Executed,Ok(v) if v.status=="unknown"=>OperationStatus::Unknown,Err(_) if plan.tool_id!="network_config"=>OperationStatus::Unknown,_=>OperationStatus::Failed};
    {
        let mut plans=crate::operation_plans()?.lock().map_err(|_|"operation state unavailable")?;
        plans.get_mut(id).ok_or("plan missing")?.status=status;
        crate::persist_operation_plans(&plans)?;
    }
    let result=outcome.map_err(|error|if status==OperationStatus::Unknown {format!("unknown: {error}")} else {error})?;
    if result.status!="completed" {return Err(format!("{}: {}",result.status,result.payload));}
    Ok(result.payload.to_string())
}

pub extern "C" fn tool_callback(tool:*const c_char,target:*const c_char,args:*const c_char,output:*mut c_char,capacity:usize,_context:*mut c_void)->i32 {
    unsafe {
        let call=std::panic::catch_unwind(||execute_tool(CStr::from_ptr(tool).to_str().unwrap_or(""),&serde_json::from_str(CStr::from_ptr(target).to_str().unwrap_or("{}")).unwrap_or(Value::Null),&serde_json::from_str(CStr::from_ptr(args).to_str().unwrap_or("{}")).unwrap_or(Value::Null)));
        let result=call.unwrap_or_else(|_|Err("native tool failed".into()));
        let payload=match result {Ok(text)=>json!({"success":true,"output":text}),Err(error)=>json!({"success":false,"output":error})}.to_string();
        copy_output(&payload,output,capacity)
    }
}
pub extern "C" fn plan_callback(target:*const c_char,tool:*const c_char,args:*const c_char,rationale:*const c_char,output:*mut c_char,capacity:usize,_context:*mut c_void)->i32 {
    unsafe {
        let result=(|| {let connection=connection(CStr::from_ptr(target).to_str().map_err(|e|e.to_string())?)?;let snapshot=CString::new(snapshot(&connection)?.to_string()).map_err(|e|e.to_string())?;
            crate::consume_result(crate::mikomai_operation_plan_create_generic(target,tool,snapshot.as_ptr(),args,rationale))})();
        match result {Ok(text)=>copy_output(&text,output,capacity),Err(error)=>{copy_output(&error,output,capacity);1}}
    }
}
unsafe fn copy_output(text:&str,output:*mut c_char,capacity:usize)->i32 {
    if output.is_null()||text.len()+1>capacity {return 1;}
    std::ptr::copy_nonoverlapping(text.as_ptr(),output as *mut u8,text.len());*output.add(text.len())=0;0
}
