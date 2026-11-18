# Run in a separate PowerShell process: attaching to the app's console must not detach CI.
# Exercises the registered Start activation, not `Start-Process gil-start.exe`.
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $AppId,
    [Parameter(Mandatory)] [string] $ExpectedExe,
    [Parameter(Mandatory)] [string] $ResultPath
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Add-Type -TypeDefinition @'
using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

[ComImport, Guid("2e941141-7f97-4756-ba1d-9decde894a3d"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IApplicationActivationManager {
    [PreserveSig] int ActivateApplication([MarshalAs(UnmanagedType.LPWStr)] string id,
        [MarshalAs(UnmanagedType.LPWStr)] string args, uint options, out uint pid);
    [PreserveSig] int ActivateForFile(IntPtr id, IntPtr items, IntPtr verb, out uint pid);
    [PreserveSig] int ActivateForProtocol(IntPtr id, IntPtr items, out uint pid);
}

public static class StartGuideCheck {
    [StructLayout(LayoutKind.Sequential)] struct Coord { public short X, Y; }
    [StructLayout(LayoutKind.Sequential)] struct Rect { public short Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] struct BufferInfo {
        public Coord Size, Cursor; public ushort Attributes; public Rect Window; public Coord Maximum;
    }
    [StructLayout(LayoutKind.Explicit, CharSet = CharSet.Unicode, Size = 20)] struct InputRecord {
        [FieldOffset(0)] public ushort EventType;
        [FieldOffset(4)] public int KeyDown;
        [FieldOffset(8)] public ushort RepeatCount;
        [FieldOffset(10)] public ushort VirtualKey;
        [FieldOffset(14)] public char Character;
    }
    [DllImport("kernel32.dll")] static extern bool FreeConsole();
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool AttachConsole(uint pid);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern IntPtr CreateFileW(string name, uint access, uint share, IntPtr security, uint mode, uint flags, IntPtr template);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr OpenProcess(uint access, bool inherit, uint pid);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool GetExitCodeProcess(IntPtr process, out uint code);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool GetConsoleScreenBufferInfo(IntPtr handle, out BufferInfo info);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool ReadConsoleOutputCharacterW(IntPtr handle, StringBuilder text, uint count, Coord origin, out uint read);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool WriteConsoleInputW(IntPtr handle, InputRecord[] records, uint count, out uint written);

    static void Check(bool ok, string what) {
        if (!ok) throw new Exception(what + ": Win32 " + Marshal.GetLastWin32Error());
    }
    static void Expect(IntPtr output, Process app, string text) {
        var timer = Stopwatch.StartNew();
        while (timer.ElapsedMilliseconds < 15000) {
            if (app.HasExited) throw new Exception("Guide exited before " + text);
            BufferInfo info;
            Check(GetConsoleScreenBufferInfo(output, out info), "console info");
            uint count = (uint)(info.Size.X * info.Size.Y);
            var buffer = new StringBuilder((int)count);
            uint read;
            Check(ReadConsoleOutputCharacterW(output, buffer, count, new Coord(), out read), "console text");
            if (buffer.ToString().Contains(text)) return;
            Thread.Sleep(100);
        }
        throw new Exception("Guide console did not display: " + text);
    }
    static void Send(IntPtr input, char choice) {
        var records = new InputRecord[] {
            new InputRecord { EventType = 1, KeyDown = 1, RepeatCount = 1, Character = choice },
            new InputRecord { EventType = 1, KeyDown = 0, RepeatCount = 1, Character = choice },
            new InputRecord { EventType = 1, KeyDown = 1, RepeatCount = 1, VirtualKey = 13, Character = '\r' },
            new InputRecord { EventType = 1, KeyDown = 0, RepeatCount = 1, VirtualKey = 13, Character = '\r' }
        };
        uint written;
        Check(WriteConsoleInputW(input, records, (uint)records.Length, out written) && written == records.Length, "console input");
    }
    public static void Run(string appId, string expectedExe) {
        var manager = (IApplicationActivationManager)Activator.CreateInstance(
            Type.GetTypeFromCLSID(new Guid("45BA127D-10A8-46EA-8AB7-56EA9078943C")));
        Process app = null;
        IntPtr processHandle = IntPtr.Zero;
        IntPtr input = new IntPtr(-1), output = new IntPtr(-1);
        try {
            uint pid;
            Marshal.ThrowExceptionForHR(manager.ActivateApplication(appId, "", 0, out pid));
            app = Process.GetProcessById((int)pid);
            // Windows, not this .NET Process object, started the app. Hold a query handle
            // while it is alive: Process.ExitCode is unavailable for this activation path.
            processHandle = OpenProcess(0x1000, false, pid); // PROCESS_QUERY_LIMITED_INFORMATION
            Check(processHandle != IntPtr.Zero, "open activated process");
            if (!String.Equals(app.MainModule.FileName, expectedExe, StringComparison.OrdinalIgnoreCase))
                throw new Exception("Start activation targeted an unexpected executable");
            FreeConsole();
            var timer = Stopwatch.StartNew();
            while (!AttachConsole(pid)) {
                if (app.HasExited || timer.ElapsedMilliseconds > 15000)
                    throw new Exception("Cannot attach to the Start guide console");
                Thread.Sleep(100);
            }
            input = CreateFileW("CONIN$", 0xC0000000, 3, IntPtr.Zero, 3, 0, IntPtr.Zero);
            output = CreateFileW("CONOUT$", 0x80000000, 3, IntPtr.Zero, 3, 0, IntPtr.Zero);
            Check(input != new IntPtr(-1) && output != new IntPtr(-1), "open console");
            Expect(output, app, "Getting started");
            Expect(output, app, "시작 안내");
            Expect(output, app, "[0] Exit");
            Send(input, '1'); Expect(output, app, "English - Connect and use GIL");
            Send(input, '2'); Expect(output, app, "한국어 - GIL 연결과 사용");
            Send(input, 'x'); Expect(output, app, "Choose 1, 2 or 0.");
            Send(input, '0');
            if (!app.WaitForExit(10000)) throw new Exception("Guide did not exit after 0");
            uint exitCode;
            Check(GetExitCodeProcess(processHandle, out exitCode), "guide exit code");
            if (exitCode != 0) throw new Exception("Guide exit code: " + exitCode);
        } finally {
            if (input != new IntPtr(-1)) CloseHandle(input);
            if (output != new IntPtr(-1)) CloseHandle(output);
            FreeConsole();
            if (app != null) { if (!app.HasExited) { app.Kill(); app.WaitForExit(); } app.Dispose(); }
            if (processHandle != IntPtr.Zero) CloseHandle(processHandle);
            Marshal.ReleaseComObject(manager);
        }
    }
}
'@
try {
    [StartGuideCheck]::Run($AppId, $ExpectedExe)
    # This verifies activation, console text and input, not the desktop's visual appearance.
    [IO.File]::WriteAllText($ResultPath, 'PASS: registered Start activation, English/Korean console, invalid input, clean exit')
} catch {
    # FreeConsole closes console handles. Preserve the failure independently of stdout/stderr.
    [IO.File]::WriteAllText($ResultPath, $_.Exception.ToString())
    exit 1
}
