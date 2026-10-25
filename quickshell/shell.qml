//@ pragma UseQApplication
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

// Standalone entry: `qs -p quickshell/shell.qml`. Draws behind windows, above the wallpaper, click-through.
// Switch preset live: `qs -p quickshell/shell.qml ipc call reactive setPreset nebula`
ShellRoot {
    id: shell

    property string preset: Quickshell.env("RD_PRESET") || "pulse"
    property real renderScale: parseFloat(Quickshell.env("RD_SCALE")) || 0.75
    property real intensity: parseFloat(Quickshell.env("RD_INTENSITY")) || 1

    AudioSource {
        id: audioSource
    }

    Variants {
        model: Quickshell.screens

        PanelWindow {
            required property ShellScreen modelData

            screen: modelData
            color: "transparent"
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.layer: WlrLayer.Bottom
            WlrLayershell.namespace: "reactive-desktop"
            mask: Region {}
            anchors {
                top: true
                bottom: true
                left: true
                right: true
            }

            ReactiveShader {
                anchors.fill: parent
                audio: audioSource
                preset: shell.preset
                renderScale: shell.renderScale
                intensity: shell.intensity
            }
        }
    }

    IpcHandler {
        target: "reactive"

        function setPreset(name: string): void {
            shell.preset = name;
        }
        function setIntensity(value: real): void {
            shell.intensity = value;
        }
        function getPreset(): string {
            return shell.preset;
        }
    }
}
