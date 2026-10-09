import QtQuick

QtObject {
    readonly property color ink: "#cdd6f4"
    readonly property color muted: "#7f849c"
    // Sub-rows of an unfolded module: between ink and muted.
    readonly property color subInk: "#a9b1cf"
    readonly property color line: "#45475a"
    readonly property color accent: "#89b4fa"
    readonly property color background: "#1e1e2e"
    // Recessed wells (tab strip), raised module blocks and hover/active fills.
    readonly property color well: "#181825"
    readonly property color surface: Qt.lighter(background, 1.16)
    readonly property color hover: "#2a2b3d"
    readonly property color active: "#313244"
    // All text sizes live here: headings < body text < control labels and values.
    readonly property font moduleHeadingFont: Qt.font({ family: "JetBrains Mono", pixelSize: 12, weight: Font.Medium })
    readonly property font settingsFont: Qt.font({ family: "JetBrains Mono", pixelSize: 13 })
    readonly property font textFont: Qt.font({ family: "JetBrains Mono", pixelSize: 12 })
}
