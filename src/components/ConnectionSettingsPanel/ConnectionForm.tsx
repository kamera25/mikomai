import React from "react";
import { useForm, Controller } from "react-hook-form";
import Select from "react-select";
import { useTranslation } from "react-i18next";
import { message, open } from "@tauri-apps/plugin-dialog";
import { Connection, McpHost } from "../../types";

const customSelectStyles = {
  control: (provided: any, state: any) => ({
    ...provided,
    backgroundColor: "var(--bg-secondary, white)",
    color: "var(--text-primary, #1e293b)",
    borderColor: state.isFocused ? "var(--accent-color, #3b82f6)" : "var(--border-color, #e2e8f0)",
    borderRadius: "6px",
    minHeight: "40px",
    fontSize: "0.95rem",
    boxShadow: state.isFocused ? "0 0 0 3px rgba(59, 130, 246, 0.1)" : "none",
    "&:hover": {
      borderColor: "var(--accent-color, #3b82f6)",
    },
  }),
  menu: (provided: any) => ({
    ...provided,
    backgroundColor: "var(--bg-secondary, white)",
    border: "1px solid var(--border-color, #e2e8f0)",
    borderRadius: "6px",
    zIndex: 9999,
  }),
  option: (provided: any, state: any) => ({
    ...provided,
    backgroundColor: state.isSelected
      ? "var(--accent-color, #3b82f6)"
      : state.isFocused
        ? "var(--bg-tertiary, #f1f5f9)"
        : "transparent",
    color: state.isSelected
      ? "white"
      : "var(--text-primary, #1e293b)",
    cursor: "pointer",
    fontSize: "0.95rem",
    "&:active": {
      backgroundColor: "var(--accent-hover, #2563eb)",
    },
  }),
  singleValue: (provided: any) => ({
    ...provided,
    color: "var(--text-primary, #1e293b)",
  }),
  input: (provided: any) => ({
    ...provided,
    color: "var(--text-primary, #1e293b)",
  }),
  placeholder: (provided: any) => ({
    ...provided,
    color: "var(--text-muted, #94a3b8)",
  }),
  dropdownIndicator: (provided: any) => ({
    ...provided,
    color: "var(--text-muted, #94a3b8)",
    "&:hover": {
      color: "var(--text-secondary, #475569)",
    },
  }),
  clearIndicator: (provided: any) => ({
    ...provided,
    color: "var(--text-muted, #94a3b8)",
  }),
  indicatorSeparator: (provided: any) => ({
    ...provided,
    backgroundColor: "var(--border-color, #e2e8f0)",
  }),
};

interface ConnectionFormProps {
  editingId: string | null;
  connections: Connection[];
  mcpHosts: McpHost[];
  deviceTypes: string[];
  getDeviceTypeAlias: (deviceType: string) => string;
  onClose: () => void;
  onSave: (
    data: any,
    isPasswordDirty: boolean,
    isEnablePasswordDirty: boolean,
    isPassphraseDirty: boolean
  ) => void;
  onDeleteCurrent: () => void;
}

export const ConnectionForm: React.FC<ConnectionFormProps> = ({
  editingId,
  connections,
  mcpHosts,
  deviceTypes,
  getDeviceTypeAlias,
  onClose,
  onSave,
  onDeleteCurrent,
}) => {
  const { t } = useTranslation();

  const editingConnection = editingId
    ? connections.find((c) => c.id === editingId)
    : null;

  const defaultValues = {
    hostname: editingConnection?.hostname || "",
    ip: editingConnection?.ip || "",
    port: editingConnection?.port ? editingConnection.port.toString() : "",
    type: (editingConnection?.type?.split(" ")[0] as any) || "SSH",
    username: editingConnection ? editingConnection.username || "" : "root",
    password: "",
    enablePassword: "",
    passphrase: "",
    rememberPassword: editingConnection?.rememberPassword ?? true,
    agentForwarding: editingConnection?.agentForwarding ?? false,
    authMethod: editingConnection?.authMethod || "plain",
    privateKeyPath: editingConnection?.privateKeyPath || "",
    consolePort: "COM1",
    baudRate: 9600,
    deviceType: editingConnection?.deviceType || "cisco_ios",
    vendorType: editingConnection?.vendorType || "",
  };

  const {
    register,
    handleSubmit,
    control,
    setValue,
    watch,
    getValues,
    formState: { errors, dirtyFields },
  } = useForm({
    defaultValues,
  });

  const connectionType = watch("type");
  const hostname = watch("hostname");

  const deviceTypeOptions = deviceTypes.map((dt) => ({
    value: dt,
    label: `${getDeviceTypeAlias(dt)} (${dt})`,
  }));

  const handleMcpLookup = async () => {
    const hostVal = getValues("hostname");
    const mcpMatch = mcpHosts.find(
      (h) => h.hostname.toLowerCase() === hostVal.toLowerCase()
    );
    if (mcpMatch) {
      let vendor = "";
      const lowerDt = mcpMatch.deviceType.toLowerCase();
      if (lowerDt.includes("cisco")) vendor = "Cisco";
      else if (lowerDt.includes("juniper")) vendor = "Juniper";
      else if (lowerDt.includes("arista")) vendor = "Arista";
      else if (lowerDt.includes("yamaha")) vendor = "Yamaha";
      else if (lowerDt.includes("linux")) vendor = "Linux";

      setValue("ip", mcpMatch.ip, { shouldDirty: true });
      setValue("type", mcpMatch.deviceType.split(" ")[0], { shouldDirty: true });
      setValue("username", mcpMatch.username, { shouldDirty: true });
      setValue("deviceType", mcpMatch.deviceType, { shouldDirty: true });
      setValue("vendorType", vendor, { shouldDirty: true });

      await message(t("connection_panel.msg_mcp_found", { hostname: mcpMatch.hostname }));
    } else {
      await message(t("connection_panel.msg_mcp_not_found", { hostname: hostVal }));
    }
  };

  const handleSelectKeyFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "SSH Key", extensions: ["*", "pem", "pub", "key", "id_rsa", "id_ed25519", "id_ecdsa"] }],
      });
      if (selected && typeof selected === "string") {
        setValue("privateKeyPath", selected, { shouldDirty: true });
      }
    } catch (e) {
      console.error("Failed to select key file:", e);
    }
  };

  const onSubmit = (data: typeof defaultValues) => {
    const isPasswordDirty = !!dirtyFields.password;
    const isEnablePasswordDirty = !!dirtyFields.enablePassword;
    const isPassphraseDirty = !!dirtyFields.passphrase;
    onSave(data, isPasswordDirty, isEnablePasswordDirty, isPassphraseDirty);
  };

  return (
    <div className="connection-form-modal-overlay">
      <form className="connection-form-card" onSubmit={handleSubmit(onSubmit)}>
        <header className="form-card-header">
          <h3>{editingId ? t("connection_panel.header_edit") : t("connection_panel.header_new")}</h3>
          <button type="button" className="close-card-btn" onClick={onClose}>
            &times;
          </button>
        </header>
        <div className="connection-form-content">
          <div className="form-section">
            <h3>{t("connection_panel.tab_common")}</h3>
            <div className="form-sub-header">{t("connection_panel.group_basic")}</div>
            <div className="form-grid">
              <div className="form-group">
                <label>{t("connection_panel.hostname_label")}</label>
                <input
                  type="text"
                  {...register("hostname")}
                  placeholder={t("connection_panel.hostname_placeholder")}
                />
                <button
                  type="button"
                  className="btn-mcp-lookup"
                  onClick={handleMcpLookup}
                  disabled={!hostname}
                  title={t("connection_panel.mcp_fetch")}
                >
                  <svg
                    width="14"
                    height="14"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path>
                    <polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline>
                    <line x1="12" y1="22.08" x2="12" y2="12"></line>
                  </svg>
                  {t("connection_panel.mcp_fetch_btn")}
                </button>
              </div>
              <div className="form-group">
                <label>{t("connection_panel.connection_type")}</label>
                <select {...register("type")}>
                  <option value="SSH">SSH</option>
                  <option value="Telnet">Telnet</option>
                  <option value="Console">Console (Serial)</option>
                </select>
              </div>

              <div className="form-group">
                <label>{t("connection_panel.vendor_type")}</label>
                <input
                  type="text"
                  {...register("vendorType")}
                  placeholder={t("connection_panel.vendor_placeholder")}
                />
              </div>
              <div className="form-group">
                <label>{t("connection_panel.device_type")}</label>
                <Controller
                  name="deviceType"
                  control={control}
                  render={({ field }) => (
                    <Select
                      options={deviceTypeOptions}
                      value={deviceTypeOptions.find((opt) => opt.value === field.value)}
                      onChange={(selectedOption) => field.onChange(selectedOption ? selectedOption.value : "")}
                      styles={customSelectStyles}
                      placeholder="Select device type..."
                    />
                  )}
                />
              </div>
            </div>

            <div className="form-sub-header" style={{ marginTop: "20px" }}>
              {t("connection_panel.group_auth")}
            </div>
            <div className="form-grid">
              <div className="form-group">
                <label>{t("connection_panel.username_label")}</label>
                <input type="text" {...register("username")} />
              </div>
              <div className="form-group">
                <label>{t("connection_panel.password_label")}</label>
                <input
                  type="password"
                  {...register("password")}
                  placeholder={editingConnection?.hasPassword ? "••••••••" : ""}
                />
              </div>
              <div className="form-group full-width">
                <label>{t("connection_panel.enable_password_label")}</label>
                <input
                  type="password"
                  {...register("enablePassword")}
                  placeholder={editingConnection?.hasEnablePassword ? "••••••••" : ""}
                />
              </div>
            </div>
          </div>

          <div className="form-section">
            <h3>{t("connection_panel.tab_endpoint")}</h3>
            <div className="form-grid">
              {connectionType !== "Console" ? (
                <>
                  <div className="form-group full-width">
                    <label>
                      {t("connection_panel.ip_label")} <span style={{ color: "#ef4444" }}>*</span>
                    </label>
                    <input
                      type="text"
                      className={errors.ip ? "error" : ""}
                      {...register("ip", {
                        validate: (val) => {
                          if (connectionType !== "Console" && !val?.trim()) {
                            return t("connection_panel.err_ip_required");
                          }
                          return true;
                        },
                      })}
                      placeholder="192.168.1.1 or router.local"
                    />
                    {errors.ip && <span className="error-message">{errors.ip.message}</span>}
                  </div>
                  <div className="form-group full-width">
                    <label>{t("connection_panel.port_label")}</label>
                    <input
                      type="text"
                      {...register("port", {
                        onChange: (e) => {
                          setValue("port", e.target.value.replace(/[^0-9]/g, ""));
                        },
                      })}
                      placeholder={
                        connectionType === "SSH" ? "22" : connectionType === "Telnet" ? "23" : ""
                      }
                    />
                  </div>
                </>
              ) : (
                <>
                  <div className="form-group">
                    <label>{t("connection_panel.serial_port_label")}</label>
                    <input
                      type="text"
                      {...register("consolePort")}
                      placeholder="COM1 or /dev/ttyUSB0"
                    />
                  </div>
                  <div className="form-group">
                    <label>{t("connection_panel.baudrate_label")}</label>
                    <select {...register("baudRate", { valueAsNumber: true })}>
                      <option value="9600">9600</option>
                      <option value="19200">19200</option>
                      <option value="38400">38400</option>
                      <option value="57600">57600</option>
                      <option value="115200">115200</option>
                    </select>
                  </div>
                </>
              )}
            </div>
          </div>

          {connectionType === "SSH" && (
            <div className="form-section">
              <h3>{t("connection_panel.tab_ssh")}</h3>
              <div className="ssh-auth-grid">
                <div className="form-group">
                  <label>{t("connection_panel.passphrase_label")}</label>
                  <input
                    type="password"
                    {...register("passphrase")}
                    placeholder={editingConnection?.hasPassphrase ? "••••••••" : ""}
                  />
                </div>

                <div className="ssh-checkbox-group">
                  <label className="checkbox-item">
                    <input type="checkbox" {...register("rememberPassword")} />
                    {t("connection_panel.remember_password")}
                  </label>
                  <label className="checkbox-item">
                    <input type="checkbox" {...register("agentForwarding")} />
                    {t("connection_panel.agent_forwarding")}
                  </label>
                </div>

                <div className="auth-methods-list">
                  <div className="auth-method-item">
                    <input
                      type="radio"
                      value="plain"
                      {...register("authMethod")}
                    />
                    <div className="auth-method-content">
                      <span className="auth-method-label">{t("connection_panel.auth_password")}</span>
                    </div>
                  </div>

                  <div className="auth-method-item">
                    <input
                      type="radio"
                      value="key"
                      {...register("authMethod")}
                    />
                    <div className="auth-method-content">
                      <span className="auth-method-label">{t("connection_panel.auth_key")}</span>
                      <div className="auth-method-details">
                        <button type="button" className="btn-file-select" onClick={handleSelectKeyFile}>
                          {t("connection_panel.key_select_btn")}
                        </button>
                        <input
                          type="text"
                          className="path-input"
                          placeholder={t("connection_panel.key_placeholder")}
                          {...register("privateKeyPath")}
                          disabled={watch("authMethod") !== "key"}
                        />
                      </div>
                    </div>
                  </div>

                  <div className="auth-method-item">
                    <input
                      type="radio"
                      value="keyboard"
                      {...register("authMethod")}
                    />
                    <div className="auth-method-content">
                      <span className="auth-method-label">
                        {t("connection_panel.auth_keyboard_interactive")}
                      </span>
                    </div>
                  </div>

                  <div className="auth-method-item">
                    <input
                      type="radio"
                      value="pageant"
                      {...register("authMethod")}
                    />
                    <div className="auth-method-content">
                      <span className="auth-method-label">{t("connection_panel.auth_pageant")}</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
        <footer className="form-footer">
          {editingId && (
            <button
              type="button"
              className="btn-cancel"
              style={{
                marginRight: "auto",
                backgroundColor: "#fee2e2",
                color: "#dc2626",
                borderColor: "#f87171",
              }}
              onClick={onDeleteCurrent}
            >
              {t("common.delete")}
            </button>
          )}
          <button type="button" className="btn-cancel" onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button type="submit" className="btn-save">
            {editingId ? t("common.save") : t("connection_panel.btn_add_host")}
          </button>
        </footer>
      </form>
    </div>
  );
};
