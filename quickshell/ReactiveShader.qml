import QtQuick

// Full-size audio-reactive shader. Renders at `renderScale` of its size and upscales, to stay cheap.
Item {
    id: root

    required property AudioSource audio
    property string preset: "pulse"
    property real renderScale: 1
    property real intensity: 1
    property color color1: "#a6c8ff"
    property color color2: "#d6bee4"
    property color color3: "#ffb4ab"

    ShaderEffect {
        id: fx

        width: root.width * root.renderScale
        height: root.height * root.renderScale
        scale: 1 / root.renderScale
        transformOrigin: Item.TopLeft
        smooth: true
        blending: true

        fragmentShader: Qt.resolvedUrl(`shaders/${root.preset}.frag.qsb`)

        property real iTime: 0
        property vector2d iResolution: Qt.vector2d(width, height)
        property real uFlow: root.audio.flow
        property real uLevel: root.audio.level
        property real uBass: root.audio.bass
        property real uMid: root.audio.mid
        property real uHigh: root.audio.high
        property real uBeat: root.audio.beat
        property vector4d uSpec0: root._bins(0)
        property vector4d uSpec1: root._bins(4)
        property vector4d uSpec2: root._bins(8)
        property vector4d uSpec3: root._bins(12)
        property color uColor1: root.color1
        property color uColor2: root.color2
        property color uColor3: root.color3
        property real uIntensity: root.intensity

        FrameAnimation {
            running: root.visible
            onTriggered: fx.iTime += frameTime
        }
    }

    function _bins(i: int): vector4d {
        const s = audio.spec;
        return Qt.vector4d(s[i], s[i + 1], s[i + 2], s[i + 3]);
    }
}
