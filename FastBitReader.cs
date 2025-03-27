using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace DisasterServer
{
    public class FastBitReader
    {
        public int Position { get; set; } = 0;

        public byte ReadByte(ref byte[] data)
        {
            if (Position >= data.Length)
                throw new ArgumentOutOfRangeException(nameof(data));

            return data[Position++];
        }

        public bool ReadBoolean(ref byte[] data)
        {
            if (Position >= data.Length)
                throw new ArgumentOutOfRangeException(nameof(data));

            return Convert.ToBoolean(data[Position++]);
        }

        public uint ReadUInt(ref byte[] data)
        {
            if (Position >= data.Length)
                throw new ArgumentOutOfRangeException(nameof(data));

            uint val = (uint)(data[Position] | ((uint)data[Position + 1] << 8) | ((uint)data[Position + 2] << 16) | ((uint)data[Position + 3] << 24));

            Position += 4;
            return val;
        }
    }
}
