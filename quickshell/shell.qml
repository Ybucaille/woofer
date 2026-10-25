//@ pragma UseQApplication
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland

// Standalone entry: `qs -p quickshell/shell.qml`. Draws behind windows, above the wallpaper, click-through.
// Switch preset live: `qs -p quickshell/shell.qml ipc call reactive setPreset nebula`
ShellRoot {
    id: shell

    property string preset: Quickshell.env("RD_PRESET") || "pulse"
    property real renderScale: parseFloat(Quickshell.env("RD_SCALE")) || 0.75
    property real intensity: parseFloat(Quickshell.env("RD_INTENSITY")) || 1
    // Colours follow the Caelestia (Material You) scheme when present; RD_COLORS=default keeps the built-in ones.
    property bool useTheme: Quickshell.env("RD_COLORS") !== "default"
    property var themeColours: null

    AudioSource {
        id: audioSource
    }

    FileView {
        path: `${Quickshell.env("HOME")}/.local/state/caelestia/scheme.json`
        watchChanges: true
        onFileChanged: reload()
        onLoaded: {
            try {
                shell.themeColours = JSON.parse(text()).colours;
            } catch (e) {
                shell.themeColours = null;
            }
        }
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
                id: fx

                readonly property var c: shell.useTheme ? shell.themeColours : null

                anchors.fill: parent
                // Stop rendering under fullscreen windows (games, video) to save GPU and battery.
                visible: !(Hyprland.monitorFor(modelData)?.activeWorkspace?.hasFullscreen ?? false)
                audio: audioSource
                preset: shell.preset
                renderScale: shell.renderScale
                intensity: shell.intensity
                color1: c ? `#${c.primary}` : "#a6c8ff"
                color2: c ? `#${c.secondary}` : "#d6bee4"
                color3: c ? `#${c.tertiary}` : "#ffb4ab"
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
