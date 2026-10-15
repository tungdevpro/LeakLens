# LeakLens

A desktop performance monitor for a single Android app. It charts CPU, memory and frame rendering times live while you navigate the app's screens, to help catch memory leaks.

## Language

### What is being monitored

**Device**:
An Android phone or emulator reachable over adb, identified by its adb serial number.
_Avoid_: Phone, emulator, target

**Target App**:
The Android package being monitored, such as `com.foo.bar`. It stays fixed for the whole Session.
_Avoid_: App (ambiguous with LeakLens itself), package, monitored app

**Target Process**:
One running instance of the Target App's main process (the one named exactly after the package), identified by its PID. A Session can see several Target Processes in a row. Secondary processes such as `:remote` are not monitored.
_Avoid_: App, PID (on its own)

**Process Restart**:
The moment a new Target Process replaces the previous one during a Session. It is shown as a marker on the charts.
_Avoid_: Crash (a restart can also come from the system killing the app or a force-stop)

### Monitoring

**Session**:
One continuous monitoring run of a Target App on a Device, from Start to Stop. Its data lives in memory and can be exported.
_Avoid_: Recording, run, trace

**Sample**:
One reading of the Target Process's metrics taken at a point in time during a Session.
_Avoid_: Snapshot, data point

**CPU Usage**:
The share of CPU time the Target Process used since the previous Sample, as a percentage of one core. 100% means one core fully busy.
_Avoid_: CPU load

**Object Counts**:
The live counts of Activities, Views and AppContexts in the Target Process. A count that keeps rising after you navigate away is the main sign of a leak.
_Avoid_: Instance counts

**Frame Budget**:
The time available to render one frame at the Device's refresh rate, such as 16.6 ms at 60 Hz or 8.3 ms at 120 Hz.
_Avoid_: Frame deadline, vsync interval

**Janky Frame**:
A frame whose total rendering time exceeded the Frame Budget.
_Avoid_: Dropped frame, slow frame

**Disconnection**:
A stretch of a Session during which the Device was unreachable. It shows as a gap in the charts, and the Session resumes when the same Device reconnects.
_Avoid_: Pause

### Navigation markers

**Screen**:
The Activity currently resumed in the Target Process.
_Avoid_: Page, view, route

**Screen Change**:
An automatic marker set when the resumed Activity changes.
_Avoid_: Navigation event, transition

**Mark**:
A marker the user adds by hand during a Session, with an optional label. It covers navigation that Screen Changes cannot see, such as Fragment or Compose destinations.
_Avoid_: Bookmark, annotation, tag

**Screen Visit**:
One arrival at a Screen or Mark during a Session, recorded with the Object Counts and memory at that moment and their change since the first visit to the same Screen within the current Target Process. A Process Restart resets that baseline.
_Avoid_: Page view, hit
