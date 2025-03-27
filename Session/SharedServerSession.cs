using DisasterServer.Data;
using ExeNet;
using System.Net;
using System.Net.Sockets;

namespace DisasterServer.Session
{
    public class SharedServerSession : TcpSession
    {
        private Server _server;

        private List<byte> _header = new();
        private List<byte> _data = new();
        private int _length = -1;
        private bool _start = false;

        private byte[] _headerData = new byte[] { (byte)'h', (byte)'P', (byte)'K', (byte)'T', (byte)0x0 };

        public SharedServerSession(Server server, TcpClient client) : base(server.SharedServer, client)
        {
            _server = server;
        }

        protected override void OnConnected()
        {
            lock (_server.Peers)
            {
                if (_server.Peers.Count >= 77)
                {
                    _server.DisconnectWithReason(this, "Server is full. (77/77)");
                    return;
                }

                var peer = new Peer()
                {
                    EndPoint = RemoteEndPoint!,

                    ID = ID,
                    Pending = true
                };
                _server.Peers.Add(ID, peer);
                Terminal.Log($"{RemoteEndPoint} (ID {ID}) connected.");
            }

            base.OnConnected();
        }

        protected override void OnDisconnected()
        {
            lock (_server.Peers)
            {
                Terminal.LogDebug($"Disconnect Stage 1");
                _server.Peers.Remove(ID, out Peer? peer);

                if (peer == null)
                    return;

                Terminal.LogDebug($"Disconnect Stage 2");
                var packet = new TcpPacket(PacketType.SERVER_PLAYER_LEFT, peer.ID);
                _server.TCPMulticast(packet, ID);

                Terminal.LogDebug($"Disconnect Stage 3");
                _server.State.PeerLeft(_server, this, peer);
                Terminal.Log($"{peer?.EndPoint} (ID {peer?.ID}) disconnected.");
            }
            base.OnDisconnected();
        }

        protected override void OnData(byte[] buffer, int length)
        {
            ProcessBytes(buffer, length);

            base.OnData(buffer, length);
        }

        protected override void Timeouted()
        {
            _server.DisconnectWithReason(this, "Connection timeout");
            base.Timeouted();
        }

        private void ProcessBytes(byte[] buffer, int length)
        {
            using var memStream = new MemoryStream(buffer, 0, length);

            while (memStream.Position < memStream.Length)
            {
                byte bt = (byte)memStream.ReadByte();

                if (_start)
                {
                    _start = false;
                    _length = bt;
                    _data.Clear();

                    Terminal.LogDebug($"Packet start {_length}");
                }
                else
                {
                    _data.Add(bt);

                    if (_data.Count >= _length && _length != -1)
                    {
                        var data = _data.ToArray();
                        using var stream = new MemoryStream(data);
                        using var reader = new BinaryReader(stream);

                        Terminal.LogDebug($"Packet recv {BitConverter.ToString(data)}");

                        try
                        {
                            if (data.Length > 256)
                            {
                                Terminal.Log("TCP overload (data.Length > 256)");
                                _server.DisconnectWithReason(this, "Packet overload > 256");
                            }
                            else
                                _server.State.PeerTCPMessage(_server, this, reader);
                        }
                        catch (Exception e)
                        {
                            OnError(e.Message);
                        }
                        _length = -1;
                        _data.Clear();
                    }
                }

                _header.Add(bt);
                if (_header.Count >= 6)
                    _header.RemoveAt(0);

                // Now check header
                if (Enumerable.SequenceEqual(_header, _headerData))
                    _start = true;
            }

            if (_data.Count < _length && _length != -1)
                Terminal.LogDebug($"Packet split, waiting for part to arrive.");
        }

        protected override void OnSocketError(SocketError error)
        {
            Terminal.Log($"Caught SocketError: {error}");

            _server.State.TCPSocketError(this, error);

            base.OnSocketError(error);
        }

        protected override void OnError(string message)
        {
            Terminal.Log($"Caught Error: {message}");

            base.OnError(message);
        }
    }
}
