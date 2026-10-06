using System;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Collections.Concurrent;
using System.Threading;
using Mikomai.Bindings;
sealed class Listener : EventListener {
    internal ConcurrentQueue<TaskEvent> Events = new();
    public void OnEvent(TaskEvent value) { Events.Enqueue(value); }
}
static class Program {
    static void Main(string[] args) {
        using var service = new MikomaiService();
        var listener = new Listener(); service.Subscribe(listener);
        var id = service.Submit(new Command.Contract(File.ReadAllText(args[0])));
        var deadline = DateTime.UtcNow.AddSeconds(20);
        Snapshot snapshot;
        do { snapshot=service.Query(new Query.Task(id)); if (DateTime.UtcNow>deadline) throw new Exception("contract timeout");Thread.Sleep(5); } while(snapshot.State!="completed" && snapshot.State!="failed");
        if (snapshot.State!="completed") throw new Exception(snapshot.Result);
        var events=snapshot.Events;
        if (!events.Select(e=>e.Seq).SequenceEqual(Enumerable.Range(1,events.Length).Select(n=>(ulong)n))) throw new Exception("event sequence gap");
        if(events.Any(e=>e.Version!=1||e.TaskId!=id)) throw new Exception("event contract mismatch");
        if(!listener.Events.Where(e=>e.TaskId==id).Select(e=>e.Seq).SequenceEqual(events.Select(e=>e.Seq))) throw new Exception("callback and query differ");
        Console.WriteLine(JsonSerializer.Serialize(new {version=1,kinds=events.Select(e=>e.Kind).ToArray(),result=snapshot.Result}));
    }
}
