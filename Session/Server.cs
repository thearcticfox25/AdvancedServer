using DisasterServer.Data;
using DisasterServer.State;
using ExeNet;
using System.Net;

namespace DisasterServer.Session
{
    public class Server
    {
        /* Status */
        public int UID = -1;
        public bool IsRunning = false;
        public DisasterServer.State.State State = new Lobby();

        /* Data */
        public Dictionary<ushort, Peer> Peers = new();

        /* Actual servers */
        public SharedServer SharedServer;

        private Thread? _thread;
        private int _hbTimer = 0;

        public Server()
        {
            SharedServer = new(this, 7606);
        }

        public void StartAsync()
        {
            if (!SharedServer.Start())
                throw new Exception("Failed to start SharedServer (TCP)");

            IsRunning = true;

            _thread = new Thread(() =>
            {
                while (IsRunning)
                {
                    DoHeartbeat();
                    Tick();

                    Thread.Sleep(15);
                }
            });

            _thread.Priority = ThreadPriority.AboveNormal;
            _thread.Name = $"Server {UID}";
            _thread.Start();
        }

        public void Tick() => State.Tick(this);
        public SharedServerSession? GetSession(ushort id) => (SharedServerSession?)SharedServer.GetSession(id);

        public void TCPSend(TcpSession? session, TcpPacket packet)
        {
            if (session == null)
                return;

            try
            {
                var arr = packet.ToArray();
                session.Send(arr, packet.Length);
            }
            catch (Exception e)
            {
                Terminal.Log($"TCPSend() Exception: {e}");
            }
        }

        public void TCPMulticast(TcpPacket packet, ushort? except = null)
        {
            try
            {
                var arr = packet.ToArray();

                lock (Peers)
                {
                    foreach (var peer in Peers)
                    {
                        if (peer.Key == except)
                            continue;

                        var session = GetSession(peer.Value.ID);

                        if (session == null)
                            continue;

                        session.Send(arr, packet.Length);
                    }
                }
            }
            catch (Exception e)
            {
                Terminal.Log($"TCPMulticast() Exception: {e}");
            }
        }

        public void Passtrough(BinaryReader reader, TcpSession sender)
        {
            Terminal.LogDebug("Passtrough()");
            // remember pos
            var pos = reader.BaseStream.Position;
            reader.BaseStream.Seek(0, SeekOrigin.Begin);

            // Now find type
            _ = reader.ReadByte();
            var type = reader.ReadByte();

            var pk = new TcpPacket((PacketType)type);

            //now write
            while (reader.BaseStream.Position < reader.BaseStream.Length)
                pk.Write(reader.ReadByte());

            TCPMulticast(pk, sender.ID);

            //now return back
            reader.BaseStream.Seek(pos, SeekOrigin.Begin);
            Terminal.LogDebug("Passtrough end()");
        }

        public void DisconnectWithReason(TcpSession? session, string reason)
        {
            Task.Run(() =>
            {
                if (session == null)
                    return;

                if (!session.IsRunning)
                    return;

                Terminal.LogDebug($"Disconnecting cuz following balls ({session.ID}): {reason}");
                Thread.CurrentThread.Name = $"Server {UID}";
                try
                {
                    var endpoint = session.RemoteEndPoint;
                    var id = session.ID;

                    lock (Peers)
                    {
                        if (!Peers.ContainsKey(id))
                        {
                            Terminal.Log($"(ID {id}) disconnect: {reason}");
                        }
                        else
                        {
                            var peer = Peers[id];
                            Terminal.Log($"{peer.Nickname} (ID {peer.ID}) disconnect: {reason}");
                        }
                    }

                    var pk = new TcpPacket(PacketType.SERVER_PLAYER_FORCE_DISCONNECT);
                    pk.Write(reason);
                    TCPSend(session, pk);
                    session.Disconnect();
                }
                catch (Exception e) { Terminal.Log($"Disconnect failed: {e}"); }
            });
        }

        public void SetState<T>() where T : DisasterServer.State.State
        {
            var obj = Activator.CreateInstance(typeof(T));

            if (obj == null)
                return;

            State = (DisasterServer.State.State)obj;
            State.Init(this);

            Terminal.Log($"Server state is {State} now");
        }

        /*public void SetState<T>(T value) where T : DisasterServer.State.State
        {
            State = value;
            State.Init(this);

            Terminal.Log($"Server state is {State} now");
        }*/

        private void DoHeartbeat()
        {
            lock (Peers) /* Ignore if no peers */
            {
                if (Peers.Count <= 0)
                    return;
            }

            if (_hbTimer++ < 2 * Ext.FRAMESPSEC)
                return;

            var pk = new TcpPacket(PacketType.SERVER_HEARTBEAT);
            TCPMulticast(pk);

            Terminal.LogDebug("Server heartbeated.");
            _hbTimer = 0;
        }
    }
}
