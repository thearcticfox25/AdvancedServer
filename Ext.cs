using DisasterServer.Data;
using DisasterServer.Session;
using ExeNet;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Net;
using System.Reflection.PortableExecutable;
using System.Text;
using System.Threading.Tasks;
using static System.Collections.Specialized.BitVector32;

namespace DisasterServer
{
    public static class Ext
    {
        public const int FRAMESPSEC = 60;
        private static Random _rand = new();

        public static string ReadStringNull(this BinaryReader reader)
        {
            List<byte> bytes = new();

            byte c;
            while (reader.BaseStream.Position < reader.BaseStream.Length && (c = reader.ReadByte()) != (byte)'\0')
                bytes.Add(c);

            return Encoding.UTF8.GetString(bytes.ToArray());
        }

        public static T? CreateOfType<T>()
        {
            return (T?)Activator.CreateInstance(typeof(T));
        }

        public static T? CreateOfType<T>(Type value)
        {
            return (T?)Activator.CreateInstance(value);
        }

        public static double Dist(double x, double y, double x2, double y2)
        {
            return Math.Sqrt(Math.Pow(x2 - x, 2) + Math.Pow(y2 - y, 2));
        }

        public static string ValidateNick(string nick)
        {
            string nick2 = nick;
            char[] pattern = new char[] { '\\', '/', '@', '|', '№', '`', '~', '&', ' ' };

            foreach(var ch in pattern)
                nick2 = nick2.Replace(ch.ToString(), "");

            if (nick2.Length <= 0 || string.IsNullOrEmpty(nick2) || string.IsNullOrWhiteSpace(nick2))
                return $"/player~ \\{_rand.Next(9999)}";
            return nick;
        }

        public static void HandleIdentity(Server server, TcpSession session, BinaryReader reader)
        {
            var ver = reader.ReadUInt16();
            var name = reader.ReadStringNull();
            var icon = reader.ReadByte();
            
            if (ver != 100)
            {
                server.DisconnectWithReason(session, $"Wrong game version ({100} required, but got {ver})");
                return;
            }
            
            if(server.Peers.Count >= 78)
                return;
            
            lock (server.Peers)
            {                    
                if (server.Peers.ContainsKey(session.ID))
                {
                    server.Peers[session.ID].Pending = false;
                    server.Peers[session.ID].Nickname = ValidateNick(name);
                    server.Peers[session.ID].Icon = icon;

                    if (server.State.AsState() == Session.State.LOBBY)
                    {
                        Terminal.Log($"{name} (ID {server.Peers[session.ID].ID}) joined.");

                        var packet = new TcpPacket(PacketType.SERVER_PLAYER_INFO);
                        packet.Write(server.Peers[session.ID].ID);
                        packet.Write(name);
                        packet.Write(icon);
                        server.TCPMulticast(packet, session.ID);
                    }
                }
            }
            Terminal.LogDebug($"Indentity recived from {(IPEndPoint?)session.RemoteEndPoint}:");

            var pak = new TcpPacket(PacketType.SERVER_IDENTITY_RESPONSE);
            pak.Write(server.State.AsState() == Session.State.LOBBY);
            pak.Write(session.ID);
            server.TCPSend(session, pak);
        }
    }
}
