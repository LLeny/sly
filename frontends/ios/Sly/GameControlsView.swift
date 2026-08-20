import SwiftUI
import SlyKit

private struct ControlButtonConfiguration {
    let label: String
    let button: SlyKit.Button
    var color: Color? = nil
}

private struct FaceButtonConfiguration {
    let top: ControlButtonConfiguration
    let trailing: ControlButtonConfiguration
    let bottom: ControlButtonConfiguration
    let leading: ControlButtonConfiguration
}

private struct CoreControlConfiguration {
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

struct GameControlsView: View {
    let player: Player

    private var configuration: CoreControlConfiguration {
        player.currentSystem.controlConfiguration
    }

    var body: some View {
        GeometryReader { geometry in
            let width = geometry.size.width
            let padSize = min(
                148,
                max(
                    88,
                    min((width - 116) / 2, geometry.size.height - 70)
                )
            )

            VStack(spacing: 24) {
                HStack(spacing: 20) {
                    shoulderButton(configuration.leftShoulder)
                    shoulderButton(configuration.rightShoulder)
                }

                HStack(spacing: 8) {
                    DirectionPad(size: padSize, onButtonChanged: setButton)

                    Spacer(minLength: 0)

                    centerControls

                    Spacer(minLength: 0)

                    FaceButtonPad(
                        size: padSize,
                        configuration: configuration.faceButtons,
                        onButtonChanged: setButton
                    )
                }
            }
            .padding(12)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
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
        .frame(height: 38)
    }

    private var centerControls: some View {
        VStack(spacing: 8) {
            ForEach(configuration.centerButtons, id: \.label) { control in
                ControlButton(
                    control: control,
                    font: .system(size: 10, weight: .semibold),
                    shape: Capsule(),
                    onButtonChanged: setButton
                )
                .frame(width: 60, height: 24)
            }
        }
    }

    private func setButton(_ button: SlyKit.Button, pressed: Bool) {
        player.core.setButton(button, pressed: pressed)
    }
}

private struct FaceButtonPad: View {
    let size: CGFloat
    let configuration: FaceButtonConfiguration
    let onButtonChanged: (SlyKit.Button, Bool) -> Void

    var body: some View {
        let buttonSize = size * 0.36

        ZStack {
            faceButton(configuration.top, size: buttonSize)
                .position(x: size / 2, y: buttonSize / 2)
            faceButton(configuration.trailing, size: buttonSize)
                .position(x: size - buttonSize / 2, y: size / 2)
            faceButton(configuration.bottom, size: buttonSize)
                .position(x: size / 2, y: size - buttonSize / 2)
            faceButton(configuration.leading, size: buttonSize)
                .position(x: buttonSize / 2, y: size / 2)
        }
        .frame(width: size, height: size)
    }

    private func faceButton(
        _ control: ControlButtonConfiguration,
        size: CGFloat
    ) -> some View {
        ControlButton(
            control: control,
            font: .system(size: 18, weight: .bold),
            shape: Circle(),
            onButtonChanged: onButtonChanged
        )
        .frame(width: size, height: size)
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

        for button in pressedButtons.subtracting(next) {
            onButtonChanged(button, false)
        }
        for button in next.subtracting(pressedButtons) {
            onButtonChanged(button, true)
        }
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
            Text(control.label)
                .font(font)
                .foregroundStyle(control.color == nil ? Color.primary : .white)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(fill(isPressed), in: shape)
                .scaleEffect(isPressed ? 0.94 : 1)
        }
    }

    private func fill(_ isPressed: Bool) -> Color {
        guard let color = control.color else { return controlFill(isPressed) }
        return isPressed ? color.opacity(0.72) : color
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
