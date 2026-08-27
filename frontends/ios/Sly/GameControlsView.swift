import SwiftUI
import SlyKit
import UIKit

private struct ControlButtonConfiguration: Sendable {
    let label: String
    let button: SlyKit.Button
    var color: Color? = nil
}

private struct FaceButtonConfiguration: Sendable {
    let top: ControlButtonConfiguration
    let trailing: ControlButtonConfiguration
    let bottom: ControlButtonConfiguration
    let leading: ControlButtonConfiguration
}

private struct CoreControlConfiguration: Sendable {
    let leftShoulder: ControlButtonConfiguration
    let rightShoulder: ControlButtonConfiguration
    let centerButtons: [ControlButtonConfiguration]
    let faceButtons: FaceButtonConfiguration
}

private extension System {
    var controlConfiguration: CoreControlConfiguration {
        switch self {
        case .snes:
            return .snes
        @unknown default:
            assertionFailure("Missing controls for a new emulator core")
            return .snes
        }
    }
}

private extension CoreControlConfiguration {
    static let snes = CoreControlConfiguration(
        leftShoulder: ControlButtonConfiguration(label: "L", button: .l),
        rightShoulder: ControlButtonConfiguration(label: "R", button: .r),
        centerButtons: [
            ControlButtonConfiguration(label: "SELECT", button: .select),
            ControlButtonConfiguration(label: "START", button: .start),
        ],
        faceButtons: FaceButtonConfiguration(
            top: ControlButtonConfiguration(
                label: "X",
                button: .x,
                color: Color(red: 0.18, green: 0.38, blue: 0.72)
            ),
            trailing: ControlButtonConfiguration(
                label: "A",
                button: .a,
                color: Color(red: 0.78, green: 0.20, blue: 0.27)
            ),
            bottom: ControlButtonConfiguration(
                label: "B",
                button: .b,
                color: Color(red: 0.88, green: 0.68, blue: 0.12)
            ),
            leading: ControlButtonConfiguration(
                label: "Y",
                button: .y,
                color: Color(red: 0.20, green: 0.56, blue: 0.34)
            )
        )
    )
}

private enum ControlsLayout {
    static let horizontalPadding: CGFloat = 16
    static let topPadding: CGFloat = 8
    static let bottomPadding: CGFloat = 58
    static let rowSpacing: CGFloat = 14
    static let padSpacing: CGFloat = 16
    static let auxiliaryHeight: CGFloat = 36
    static let padInset: CGFloat = 10
    static let minimumPadSize: CGFloat = 104
    static let maximumPadSize: CGFloat = 176

    static var reservedWidth: CGFloat {
        horizontalPadding * 2 + padSpacing
    }

    static var reservedHeight: CGFloat {
        topPadding + auxiliaryHeight + rowSpacing + bottomPadding + padInset
    }

    static func padSize(in size: CGSize) -> CGFloat {
        min(
            maximumPadSize,
            max(
                minimumPadSize,
                min(
                    (size.width - reservedWidth) / 2,
                    size.height - reservedHeight
                )
            )
        )
    }
}

struct GameControlsView: View {
    let player: Player

    private var configuration: CoreControlConfiguration {
        player.currentSystem.controlConfiguration
    }

    var body: some View {
        GeometryReader { geometry in
            let padSize = ControlsLayout.padSize(in: geometry.size)

            VStack(spacing: ControlsLayout.rowSpacing) {
                auxiliaryControls

                HStack(spacing: ControlsLayout.padSpacing) {
                    DirectionPad(size: padSize, onButtonChanged: setButton)

                    Spacer(minLength: 0)

                    FaceButtonPad(
                        size: padSize,
                        configuration: configuration.faceButtons,
                        onButtonChanged: setButton
                    )
                }
            }
            .padding(.horizontal, ControlsLayout.horizontalPadding)
            .padding(.top, ControlsLayout.topPadding)
            .padding(.bottom, ControlsLayout.bottomPadding)
            .frame(
                maxWidth: .infinity,
                maxHeight: .infinity,
                alignment: .bottom
            )
        }
    }

    private var auxiliaryControls: some View {
        HStack(spacing: 12) {
            shoulderButton(configuration.leftShoulder)

            HStack(spacing: 8) {
                ForEach(configuration.centerButtons, id: \.label) { control in
                    centerButton(control)
                }
            }

            shoulderButton(configuration.rightShoulder)
        }
    }

    private func shoulderButton(
        _ control: ControlButtonConfiguration
    ) -> some View {
        ControlButton(
            control: control,
            font: .system(size: 16, weight: .semibold),
            shape: Capsule(),
            onButtonChanged: setButton
        )
        .frame(maxWidth: .infinity)
        .frame(height: ControlsLayout.auxiliaryHeight)
    }

    private func centerButton(
        _ control: ControlButtonConfiguration
    ) -> some View {
        ControlButton(
            control: control,
            font: .system(size: 9, weight: .semibold),
            shape: Capsule(),
            onButtonChanged: setButton
        )
        .frame(width: 54, height: 30)
    }

    private func setButton(_ button: SlyKit.Button, pressed: Bool) {
        player.core.setButton(button, pressed: pressed)
    }
}

private struct FaceButtonPlacement {
    let control: ControlButtonConfiguration
    let center: CGPoint
}

private struct FaceButtonLayout {
    static let buttonSizeRatio: CGFloat = 0.38

    let size: CGFloat
    let configuration: FaceButtonConfiguration

    var buttonSize: CGFloat {
        size * Self.buttonSizeRatio
    }

    var placements: [FaceButtonPlacement] {
        [
            FaceButtonPlacement(
                control: configuration.top,
                center: CGPoint(x: size / 2, y: buttonSize / 2)
            ),
            FaceButtonPlacement(
                control: configuration.trailing,
                center: CGPoint(x: size - buttonSize / 2, y: size / 2)
            ),
            FaceButtonPlacement(
                control: configuration.bottom,
                center: CGPoint(x: size / 2, y: size - buttonSize / 2)
            ),
            FaceButtonPlacement(
                control: configuration.leading,
                center: CGPoint(x: buttonSize / 2, y: size / 2)
            ),
        ]
    }
}

private struct FaceButtonPad: View {
    let size: CGFloat
    let configuration: FaceButtonConfiguration
    let onButtonChanged: (SlyKit.Button, Bool) -> Void

    @State private var pressedButtons: Set<SlyKit.Button> = []

    var body: some View {
        let layout = FaceButtonLayout(size: size, configuration: configuration)

        ZStack {
            ForEach(layout.placements, id: \.control.button) { placement in
                faceButton(placement.control, size: layout.buttonSize)
                    .position(placement.center)
            }

            FaceButtonTouchSurface(
                layout: layout,
                onButtonChanged: onButtonChanged,
                onPressedButtonsChanged: { pressedButtons = $0 }
            )
            .accessibilityHidden(true)
        }
        .frame(width: size, height: size)
    }

    private func faceButton(
        _ control: ControlButtonConfiguration,
        size: CGFloat
    ) -> some View {
        ControlButtonLabel(
            control: control,
            font: .system(size: 20, weight: .bold),
            shape: Circle(),
            isPressed: pressedButtons.contains(control.button)
        )
        .allowsHitTesting(false)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(control.label)
        .accessibilityAddTraits(.isButton)
        .frame(width: size, height: size)
    }
}

private struct FaceButtonTouchSurface: UIViewRepresentable {
    let layout: FaceButtonLayout
    let onButtonChanged: (SlyKit.Button, Bool) -> Void
    let onPressedButtonsChanged: (Set<SlyKit.Button>) -> Void

    func makeUIView(context: Context) -> FaceButtonTouchView {
        let view = FaceButtonTouchView()
        update(view)
        return view
    }

    func updateUIView(_ view: FaceButtonTouchView, context: Context) {
        update(view)
    }

    static func dismantleUIView(
        _ view: FaceButtonTouchView,
        coordinator: Void
    ) {
        view.releaseAllButtons()
    }

    private func update(_ view: FaceButtonTouchView) {
        view.layout = layout
        view.onButtonChanged = onButtonChanged
        view.onPressedButtonsChanged = onPressedButtonsChanged
    }
}

private final class FaceButtonTouchView: UIView {
    var layout: FaceButtonLayout?
    var onButtonChanged: ((SlyKit.Button, Bool) -> Void)?
    var onPressedButtonsChanged: ((Set<SlyKit.Button>) -> Void)?

    private var trackedTouches: [ObjectIdentifier: Set<SlyKit.Button>] = [:]
    private var pressedButtons: Set<SlyKit.Button> = []

    override init(frame: CGRect) {
        super.init(frame: frame)
        isMultipleTouchEnabled = true
        isExclusiveTouch = false
        backgroundColor = .clear
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        update(touches)
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        update(touches)
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        remove(touches)
    }

    override func touchesCancelled(
        _ touches: Set<UITouch>,
        with event: UIEvent?
    ) {
        remove(touches)
    }

    func releaseAllButtons() {
        trackedTouches.removeAll()
        updatePressedButtons([])
    }

    private func update(_ touches: Set<UITouch>) {
        for touch in touches {
            trackedTouches[ObjectIdentifier(touch)] = buttons(for: touch)
        }
        updatePressedButtons(allTrackedButtons)
    }

    private func remove(_ touches: Set<UITouch>) {
        for touch in touches {
            trackedTouches.removeValue(forKey: ObjectIdentifier(touch))
        }
        updatePressedButtons(allTrackedButtons)
    }

    private var allTrackedButtons: Set<SlyKit.Button> {
        trackedTouches.values.reduce(into: []) { buttons, touchButtons in
            buttons.formUnion(touchButtons)
        }
    }

    private func updatePressedButtons(_ next: Set<SlyKit.Button>) {
        guard next != pressedButtons else { return }

        emitButtonChanges(from: pressedButtons, to: next) { button, pressed in
            onButtonChanged?(button, pressed)
        }
        pressedButtons = next
        onPressedButtonsChanged?(next)
    }

    private func buttons(for touch: UITouch) -> Set<SlyKit.Button> {
        guard let layout else { return [] }

        let buttonSize = layout.buttonSize
        let contactRadius = min(
            max(touch.majorRadius, buttonSize * 0.18),
            buttonSize * 0.27
        )
        let hitRadius = buttonSize / 2 + contactRadius
        let location = touch.location(in: self)

        var buttons: Set<SlyKit.Button> = []
        for placement in layout.placements {
            let distance = hypot(
                location.x - placement.center.x,
                location.y - placement.center.y
            )
            if distance <= hitRadius {
                buttons.insert(placement.control.button)
            }
        }
        return buttons
    }
}

private struct DirectionPad: View {
    let size: CGFloat
    let onButtonChanged: (SlyKit.Button, Bool) -> Void

    @State private var pressedButtons: Set<SlyKit.Button> = []

    var body: some View {
        let unit = size / 3

        ZStack {
            DPadShape()
                .fill(controlFill(false))

            Circle()
                .fill(.primary.opacity(0.1))
                .frame(width: unit * 0.65, height: unit * 0.65)

            directionSegment("▲", button: .up, unit: unit)
                .position(x: size / 2, y: unit / 2)
            directionSegment("▶", button: .right, unit: unit)
                .position(x: size - unit / 2, y: size / 2)
            directionSegment("▼", button: .down, unit: unit)
                .position(x: size / 2, y: size - unit / 2)
            directionSegment("◀", button: .left, unit: unit)
                .position(x: unit / 2, y: size / 2)
        }
        .frame(width: size, height: size)
        .contentShape(.rect)
        .gesture(
            DragGesture(minimumDistance: 0, coordinateSpace: .local)
                .onChanged { value in
                    updateDirections(at: value.location)
                }
                .onEnded { _ in
                    releaseDirections()
                }
        )
        .onDisappear {
            releaseDirections()
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Directional pad")
    }

    private func directionSegment(
        _ label: String,
        button: SlyKit.Button,
        unit: CGFloat
    ) -> some View {
        Text(label)
            .font(.system(size: unit * 0.3, weight: .bold))
            .foregroundStyle(.primary.opacity(0.78))
            .frame(width: unit, height: unit)
            .background {
                RoundedRectangle(cornerRadius: unit * 0.18)
                    .fill(
                        pressedButtons.contains(button)
                            ? controlFill(true)
                            : .clear
                    )
            }
    }

    private func updateDirections(at location: CGPoint) {
        let delta = CGVector(
            dx: location.x - size / 2,
            dy: location.y - size / 2
        )
        let deadZone = size * 0.13
        var next: Set<SlyKit.Button> = []

        if delta.dx < -deadZone { next.insert(.left) }
        if delta.dx > deadZone { next.insert(.right) }
        if delta.dy < -deadZone { next.insert(.up) }
        if delta.dy > deadZone { next.insert(.down) }

        setPressedButtons(next)
    }

    private func setPressedButtons(_ next: Set<SlyKit.Button>) {
        guard next != pressedButtons else { return }

        emitButtonChanges(from: pressedButtons, to: next, onButtonChanged)
        pressedButtons = next
    }

    private func releaseDirections() {
        setPressedButtons([])
    }
}

private struct DPadShape: Shape {
    func path(in rect: CGRect) -> Path {
        let unit = min(rect.width, rect.height) / 3
        let corner = unit * 0.18
        let cornerSize = CGSize(width: corner, height: corner)
        var path = Path()

        path.addRoundedRect(
            in: CGRect(
                x: rect.minX,
                y: rect.midY - unit / 2,
                width: rect.width,
                height: unit
            ),
            cornerSize: cornerSize
        )
        path.addRoundedRect(
            in: CGRect(
                x: rect.midX - unit / 2,
                y: rect.minY,
                width: unit,
                height: rect.height
            ),
            cornerSize: cornerSize
        )
        return path
    }
}

private func controlFill(_ isPressed: Bool) -> Color {
    .primary.opacity(isPressed ? 0.22 : 0.12)
}

private func emitButtonChanges(
    from current: Set<SlyKit.Button>,
    to next: Set<SlyKit.Button>,
    _ onButtonChanged: (SlyKit.Button, Bool) -> Void
) {
    for button in current.subtracting(next) {
        onButtonChanged(button, false)
    }
    for button in next.subtracting(current) {
        onButtonChanged(button, true)
    }
}

private extension ControlButtonConfiguration {
    func fill(isPressed: Bool) -> Color {
        guard let color else { return controlFill(isPressed) }
        return isPressed ? color.opacity(0.72) : color
    }
}

private struct ControlButtonLabel<ControlShape: Shape>: View {
    let control: ControlButtonConfiguration
    let font: Font
    let shape: ControlShape
    let isPressed: Bool

    var body: some View {
        Text(control.label)
            .font(font)
            .foregroundStyle(control.color == nil ? Color.primary : .white)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(control.fill(isPressed: isPressed), in: shape)
            .scaleEffect(isPressed ? 0.94 : 1)
    }
}

private struct ControlButton<ControlShape: Shape>: View {
    let control: ControlButtonConfiguration
    let font: Font
    let shape: ControlShape
    let onButtonChanged: (SlyKit.Button, Bool) -> Void

    var body: some View {
        PressableControl(
            label: control.label,
            onPressedChanged: { pressed in
                onButtonChanged(control.button, pressed)
            }
        ) { isPressed in
            ControlButtonLabel(
                control: control,
                font: font,
                shape: shape,
                isPressed: isPressed
            )
        }
    }
}

private struct PressableControl<Label: View>: View {
    let label: String
    let onPressedChanged: (Bool) -> Void
    @ViewBuilder let content: (Bool) -> Label

    @State private var isPressed = false

    var body: some View {
        content(isPressed)
            .contentShape(.rect)
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { _ in setPressed(true) }
                    .onEnded { _ in setPressed(false) }
            )
            .onDisappear {
                setPressed(false)
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(label)
            .accessibilityAddTraits(.isButton)
    }

    private func setPressed(_ pressed: Bool) {
        guard pressed != isPressed else { return }
        isPressed = pressed
        onPressedChanged(pressed)
    }
}
