using ExeNet;
using System.Net;
using System.Net.Sockets;

namespace DisasterServer.Session
{
    /// <summary>
    /// Server used for important packets (Time sync, entity destruction, etc)
    /// 
    /// </summary>
    public class SharedServer : TcpServer
    {
        protected Server _server;

        public SharedServer(Server server, int port) : base(IPAddress.Any, port)
        {
            _server = server;
        }

        protected override void OnReady()
        {
            Terminal.Log($"Server started on TCP port {Port}");

            base.OnReady();
        }

        protected override void OnSocketError(SocketError error)
        {
            Terminal.Log($"Caught SocketError: {error}");

            base.OnSocketError(error);
        }

        protected override void OnError(string message)
        {
            Terminal.Log($"Caught Error: {message}");

            base.OnError(message);
        }

        protected override TcpSession CreateSession(TcpClient client)
        {
            return new SharedServerSession(_server, client);
        }
    }
}
