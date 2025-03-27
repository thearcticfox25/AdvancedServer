using DisasterServer.Data;
using DisasterServer.Session;
using ExeNet;
using System.Linq;
using System.Net;
using System.Numerics;

namespace DisasterServer.State
{
    public class Lobby : State
    {
        private int _timeout = 1 * Ext.FRAMESPSEC;
        private Random _rand = new();
        private Dictionary<ushort, int> _lastPackets = new();
        public override Session.State AsState()
        {
            return Session.State.LOBBY;
        }

        public override void PeerJoined(Server server, TcpSession session, Peer peer)
        {
            lock (_lastPackets)
                _lastPackets.Add(peer.ID, 0);

            peer.ExeChance = _rand.Next(2, 5);

            var packet = new TcpPacket(PacketType.SERVER_LOBBY_EXE_CHANCE, (byte)peer.ExeChance);
            server.TCPSend(session, packet);

            packet = new TcpPacket(PacketType.SERVER_PLAYER_JOINED, peer.ID);
            server.TCPMulticast(packet, peer.ID);
        }

        public override void PeerLeft(Server server, TcpSession session, Peer peer)
        {
            lock (server.Peers)
            {
                lock (_lastPackets)
                    _lastPackets.Remove(peer.ID);

                Terminal.Log($"{peer.Nickname} (ID {peer.ID}) left.");
            }
        }

        public override void PeerTCPMessage(Server server, TcpSession session, BinaryReader reader)
        {
            var passtrough = reader.ReadBoolean();
            var type = reader.ReadByte();

            if (passtrough)
                server.Passtrough(reader, session);

            lock (_lastPackets)
                _lastPackets[session.ID] = 0;

            switch ((PacketType)type)
            {
                case PacketType.IDENTITY:
                    {
                        Ext.HandleIdentity(server, session, reader);
                        break;
                    }

                /* Player requests player list */
                case PacketType.CLIENT_LOBBY_PLAYERS_REQUEST:
                    {
                        lock (server.Peers)
                        {
                            foreach (var player in server.Peers)
                            {
                                if (player.Value.Pending)
                                    continue;

                                if (player.Key == session.ID)
                                    continue;

                                Terminal.LogDebug($"Sending {player.Value.Nickname}'s data to PID {session.ID}");

                                var pk = new TcpPacket(PacketType.SERVER_LOBBY_PLAYER);
                                pk.Write(player.Value.ID);
                                pk.Write(player.Value.IsReady);
                                pk.Write(player.Value.Nickname[..Math.Min(player.Value.Nickname.Length, 15)]);
                                pk.Write(player.Value.Icon);

                                server.TCPSend(session, pk);
                            }
                        }

                        server.TCPSend(session, new TcpPacket(PacketType.SERVER_LOBBY_CORRECT));
                        SendMessage(server, session, "|type .help for command list~");
                        if(server.Peers.Count >= 17){
                            SendMessage(server, session, "\\warning: the server might be overloaded");
                            SendMessage(server, session, $"\\there are currently {server.Peers.Count} players out of 77.");
                        }
                        break;
                    }

                /* Chat message */
                case PacketType.CLIENT_CHAT_MESSAGE:
                    {
                        var id = reader.ReadUInt16();
                        var msg = reader.ReadStringNull();

                        lock (server.Peers)
                        {
                            switch (msg)
                            {
                                case ".help":
                                case ".h":
                                    SendMessage(server, session, $"~---|list of commands:~---");
                                    SendMessage(server, session, "@.mute~ (.m) - toggle chat messages");
                                    SendMessage(server, session, "@.info~ (.i) - display a server info");
                                    SendMessage(server, session, "@.rules~ (.r) - display a server rules");
                                    SendMessage(server, session, $"~----------------------");
                                    break;


                                case ".info":
                                case ".i":
                                    SendMessage(server, session, $"~----/disaster\\server:~----");
                                    SendMessage(server, session, "@original by:~ team exe empire");
                                    SendMessage(server, session, "@rebuilt by:~ theslavonicfox");
                                    SendMessage(server, session, "@with a goal of~ creating friendly chat rooms");
                                    SendMessage(server, session, "@\\warning: happy april fools day");
                                    SendMessage(server, session, $"~----------------------");
                                    break;

                                case ".rules":
                                case ".r":
                                    SendMessage(server, session, $"~--------|rules:~--------");
                                    SendMessage(server, session, "@this server is for friendly chat)~");
                                    SendMessage(server, session, "@don't bully each-other~");
                                    SendMessage(server, session, "@consider using polite language~");
                                    SendMessage(server, session, "@don't be rude or offensive~");
                                    SendMessage(server, session, "@don't lie~");
                                    SendMessage(server, session, $"~----------------------");
                                    break;

                                default:
                                    foreach (var peer in server.Peers.Values)
                                    {
                                        if (peer.ID != id)
                                            continue;

                                        Terminal.Log($"[{peer.Nickname}]: {msg}");
                                    }
                                    break;
                            }
                        }
                        break;
                    }

                /* New ready state (key Z) */
                case PacketType.CLIENT_LOBBY_READY_STATE:
                    {
                        var ready = reader.ReadBoolean();

                        lock (server.Peers)
                        {
                            if (!server.Peers.ContainsKey(session.ID))
                                break;

                            var peer = server.Peers[session.ID];
                            peer.IsReady = ready;

                            var pk = new TcpPacket(PacketType.SERVER_LOBBY_READY_STATE);
                            pk.Write(peer.ID);
                            pk.Write(ready);
                            server.TCPMulticast(pk, session.ID);
                        }
                        break;
                    }
            }
        }

        public override void Init(Server server)
        {
            lock (server.Peers)
            {
                foreach (var peer in server.Peers.Values)
                {
                    var pk = new TcpPacket(PacketType.SERVER_GAME_BACK_TO_LOBBY);
                    server.TCPSend(server.GetSession(peer.ID), pk);

                    lock (_lastPackets)
                        _lastPackets.Add(peer.ID, 0);

                    if (peer.ExeChance >= 99)
                        peer.ExeChance = 99;

                    var packet = new TcpPacket(PacketType.SERVER_LOBBY_EXE_CHANCE, (byte)peer.ExeChance);
                    server.TCPSend(server.GetSession(peer.ID), packet);
                }
            }
        }

        public override void Tick(Server server)
        {
            DoTimeout(server);
        }

        private void DoTimeout(Server server)
        {
            if (_timeout-- > 0)
                return;

            lock (server.Peers)
            {
                lock (_lastPackets)
                {
                    foreach (var peer in server.Peers.Values)
                    {
                        if (!_lastPackets.ContainsKey(peer.ID))
                            continue;

                        if (peer.IsReady)
                        {
                            _lastPackets[peer.ID] = 0;
                            continue;
                        }

                        if (_lastPackets[peer.ID] >= 25 * Ext.FRAMESPSEC)
                        {
                            server.DisconnectWithReason(server.GetSession(peer.ID), "AFK or Timeout");
                            continue;
                        }

                        _lastPackets[peer.ID] += Ext.FRAMESPSEC;
                    }
                }
            }

            _timeout = 1 * Ext.FRAMESPSEC;
        }

        private void SendMessage(Server server, TcpSession session, string text)
        {
            var pack = new TcpPacket(PacketType.CLIENT_CHAT_MESSAGE, (ushort)0);
            pack.Write(text);
            server.TCPSend(session, pack);
        }
    }
}
