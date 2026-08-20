import CoreImage
import MetalKit
import SwiftUI
import SlyKit

struct MetalPlayerView: UIViewRepresentable {
    private static let bufferedFrameCount = 3

    let player: Player
    let framesPerSecond: Int
    let isPaused: Bool

    func makeCoordinator() -> Renderer {
        Renderer(player: player, framesPerSecond: framesPerSecond)
    }

    func makeUIView(context: Context) -> MTKView {
        let view = MTKView(frame: .zero, device: context.coordinator.device)
        view.delegate = context.coordinator
        view.colorPixelFormat = .bgra8Unorm
        view.framebufferOnly = false
        (view.layer as? CAMetalLayer)?.maximumDrawableCount =
            Self.bufferedFrameCount
        view.enableSetNeedsDisplay = false
        return view
    }

    func updateUIView(_ view: MTKView, context: Context) {
        context.coordinator.update(
            player: player,
            framesPerSecond: framesPerSecond,
            isPaused: isPaused
        )
        view.isPaused = isPaused
        view.preferredFramesPerSecond = framesPerSecond
    }

    static func dismantleUIView(_ view: MTKView, coordinator: Renderer) {
        view.isPaused = true
        view.delegate = nil
        coordinator.stop()
    }

    final class Renderer: NSObject, MTKViewDelegate {
        private var player: Player
        private var framesPerSecond: Int

        let device: MTLDevice

        private let commandQueue: MTLCommandQueue
        private let context: CIContext
        private var audioOutput: AudioOutput?
        private let colorSpace = CGColorSpace(name: CGColorSpace.sRGB)!
        private var frameTextures: [(texture: MTLTexture, image: CIImage)] = []
        private var nextFrameTexture = 0

        init(player: Player, framesPerSecond: Int) {
            guard let device = MTLCreateSystemDefaultDevice(),
                  let commandQueue = device.makeCommandQueue()
            else {
                fatalError("Metal is unavailable")
            }

            self.player = player
            self.framesPerSecond = framesPerSecond
            self.device = device
            self.commandQueue = commandQueue
            context = CIContext(
                mtlDevice: device,
                options: [.cacheIntermediates: false]
            )
            audioOutput = AudioOutput(
                core: player.core,
                framesPerSecond: framesPerSecond
            )
        }

        func draw(in view: MTKView) {
            guard let drawable = view.currentDrawable,
                  let frame = player.runFrame()
            else {
                return
            }

            audioOutput?.enqueue(from: player.core)

            guard let image = upload(frame),
                  let commandBuffer = commandQueue.makeCommandBuffer()
            else {
                return
            }

            let bounds = CGRect(
                origin: .zero,
                size: CGSize(
                    width: view.drawableSize.width,
                    height: view.drawableSize.height
                )
            )
            guard !bounds.isEmpty else { return }

            let scale = min(
                bounds.width / image.extent.width,
                bounds.height / image.extent.height
            )
            let transform = CGAffineTransform(
                a: scale,
                b: 0,
                c: 0,
                d: scale,
                tx: (bounds.width - image.extent.width * scale) / 2,
                ty: (bounds.height - image.extent.height * scale) / 2
            )
            let background = CIImage(color: .black).cropped(to: bounds)
            let output = image
                .transformed(by: transform)
                .composited(over: background)

            context.render(
                output,
                to: drawable.texture,
                commandBuffer: commandBuffer,
                bounds: bounds,
                colorSpace: colorSpace
            )
            commandBuffer.present(drawable)
            commandBuffer.commit()
        }

        func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {}

        func update(player: Player, framesPerSecond: Int, isPaused: Bool) {
            if self.player !== player ||
                self.framesPerSecond != framesPerSecond {
                audioOutput?.stop()
                self.player = player
                self.framesPerSecond = framesPerSecond
                audioOutput = AudioOutput(
                    core: player.core,
                    framesPerSecond: framesPerSecond
                )
            }

            audioOutput?.setPaused(isPaused)
        }

        func stop() {
            audioOutput?.stop()
            audioOutput = nil
        }

        private func upload(_ frame: Frame) -> CIImage? {
            guard frame.width > 0,
                  frame.height > 0,
                  frame.rgba.count == frame.width * frame.height * 4
            else {
                return nil
            }

            if frameTextures.first?.texture.width != frame.width ||
                frameTextures.first?.texture.height != frame.height {
                let descriptor = MTLTextureDescriptor.texture2DDescriptor(
                    pixelFormat: .rgba8Unorm,
                    width: frame.width,
                    height: frame.height,
                    mipmapped: false
                )
                descriptor.storageMode = .shared
                descriptor.usage = .shaderRead
                frameTextures = (0..<MetalPlayerView.bufferedFrameCount)
                    .compactMap { _ in
                        guard let texture = device.makeTexture(
                            descriptor: descriptor
                        ),
                              let image = CIImage(
                                  mtlTexture: texture,
                                  options: [.colorSpace: colorSpace]
                              )
                        else {
                            return nil
                        }

                        return (
                            texture,
                            image.transformed(
                                by: CGAffineTransform(
                                    a: 1,
                                    b: 0,
                                    c: 0,
                                    d: -1,
                                    tx: 0,
                                    ty: CGFloat(frame.height)
                                )
                            )
                        )
                    }
                nextFrameTexture = 0
            }

            guard frameTextures.count == MetalPlayerView.bufferedFrameCount
            else {
                return nil
            }

            let frameTexture = frameTextures[nextFrameTexture]
            nextFrameTexture =
                (nextFrameTexture + 1) % MetalPlayerView.bufferedFrameCount

            frame.rgba.withUnsafeBytes { bytes in
                guard let address = bytes.baseAddress else { return }
                frameTexture.texture.replace(
                    region: MTLRegionMake2D(0, 0, frame.width, frame.height),
                    mipmapLevel: 0,
                    withBytes: address,
                    bytesPerRow: frame.width * 4
                )
            }
            return frameTexture.image
        }
    }
}
