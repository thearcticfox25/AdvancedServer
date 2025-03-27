using DisasterServer.Data;
using DisasterServer.Session;
using ExeNet;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Threading.Tasks;

namespace DisasterServer.State
{
    public abstract class State
    {
        /// <summary>
        /// Called when TCP connection is established with a peer
        /// </summary>
        /// <param name="server"></param>
        /// <param name="session"></param>
        /// <param name="peer"></param>
        public abstract void PeerJoined(Server server, TcpSession session, Peer peer);

        /// <summary>
        /// Called when TCP connection reset with a peer
        /// </summary>
        /// <param name="server"></param>
        /// <param name="session"></param>
        /// <param name="peer"></param>
        public abstract void PeerLeft(Server server, TcpSession session, Peer peer);

        /// <summary>
        /// Called when TCP packet received from a peer
        /// </summary>
        /// <param name="server"></param>
        /// <param name="session"></param>
        /// <param name="reader"></param>
        public abstract void PeerTCPMessage(Server server, TcpSession session, BinaryReader reader);

        /// <summary>
        /// Called if error was caught while sending/receiving data from TCP socket
        /// </summary>
        /// <param name="session"></param>
        /// <param name="error"></param>
        public virtual void TCPSocketError(TcpSession session, SocketError error) { }

        /// <summary>
        /// Called once when setting new state
        /// </summary>
        /// <param name="server"></param>
        public abstract void Init(Server server);

        /// <summary>
        /// Called 60 times a second
        /// </summary>
        /// <param name="server"></param>
        public abstract void Tick(Server server);

        /// <summary>
        /// State as <see cref="Session.State"/> enum
        /// </summary>
        /// <returns> State as enum </returns>
        public abstract Session.State AsState();
    }
}
