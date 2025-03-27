using System.Net;

namespace DisasterServer.Data
{
    public class Peer
    {
        public ushort ID = 0;
        public string Nickname = "Pending...";
        public int ExeChance = 0;
        public byte Icon = 0;
        public bool Pending = true;
        public EndPoint EndPoint;
        public bool IsReady = false;
    }
}
