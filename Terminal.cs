using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

namespace DisasterServer
{
    public class Terminal
    {
        static Terminal()
        {
            try
            {
                AppDomain currentDomain = AppDomain.CurrentDomain;
            }
            catch
            {
                Console.WriteLine("Initialisation failed.");
            }
        }

        public static void Log(string text)
        {
            var time = DateTime.Now.ToLongTimeString();
            var msg = $"[{time} INFO] {text}";

            Console.WriteLine(msg);
        }

        public static void LogDebug(string text)
        {
            var time = DateTime.Now.ToLongTimeString();
            var msg = $"[{time} DEBUG] {text}";

            Console.WriteLine(msg);
        }
    }
}
