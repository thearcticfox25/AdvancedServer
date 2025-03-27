using DisasterServer.Session;
using DisasterServer.State;

namespace DisasterServer
{
    public class Program
    {
        public static void Main(string[] args)
        {
            Terminal.Log("===================");
            Terminal.Log("DisasterServer");
            Terminal.Log("Built by TheSlavonicFox");
            Terminal.Log("===================");
            Terminal.Log("Original files belong to:");
            Terminal.Log("(c) Team Exe Empire 2023");
            Terminal.Log("===================");

            Server server = new();
            server.StartAsync();

            while (true) Thread.Sleep(1000);
        }
    }
}