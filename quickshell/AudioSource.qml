import QtQuick
import Quickshell
import Quickshell.Io
import "analysis.js" as A

// Runs cava on PipeWire output and exposes analysed audio for ReactiveShader.
Scope {
    id: root

    readonly property int bands: 16
    property real bass
    property real mid
    property real high
    property real level
    property real beat
    property real flow
    property var spec: new Array(bands).fill(0)

    property var _raw: new Array(bands).fill(0)
    property var _state: A.rdCreateState(bands)

    Process {
        id: cava

        running: true
        command: ["cava", "-p", Quickshell.shellPath("cava.conf")]
        stdout: SplitParser {
            onRead: data => {
                const parts = data.split(";");
                const raw = root._raw;
                for (let i = 0; i < root.bands; i++)
                    raw[i] = (parseInt(parts[i]) || 0) / 1000;
            }
        }
        onExited: restart.start()
    }

    Timer {
        id: restart

        interval: 2000
        onTriggered: cava.running = true
    }

    FrameAnimation {
        running: true
        onTriggered: {
            const s = A.rdAnalyse(root._state, root._raw, frameTime);
            root.bass = s.bass;
            root.mid = s.mid;
            root.high = s.high;
            root.level = s.level;
            root.beat = s.beat;
            root.flow = s.flow;
            root.spec = s.spec.slice();
        }
    }
}
