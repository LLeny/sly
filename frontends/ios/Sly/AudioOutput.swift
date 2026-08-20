import AVFoundation
import SlyKit

final class AudioOutput {
    private static let int16FullScale = Float(Int16.max) + 1

    private let engine = AVAudioEngine()
    private let playerNode = AVAudioPlayerNode()
    private let format: AVAudioFormat
    private let channelCount: Int
    private let maximumSamplesPerFrame: Int

    init?(core: EmulatorCore, framesPerSecond: Int) {
        guard let spec = core.audioSpec else { return nil }

        channelCount = Int(spec.channels)
        guard spec.sampleRate > 0,
              channelCount > 0,
              framesPerSecond > 0,
              let format = AVAudioFormat(
                  standardFormatWithSampleRate: Double(spec.sampleRate),
                  channels: AVAudioChannelCount(channelCount)
              )
        else {
            return nil
        }

        self.format = format
        let maximumAudioFrames =
            (Int(spec.sampleRate) + framesPerSecond - 1) / framesPerSecond
        maximumSamplesPerFrame = maximumAudioFrames * channelCount

        do {
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.ambient, mode: .default)
            try session.setActive(true)

            engine.attach(playerNode)
            engine.connect(playerNode, to: engine.mainMixerNode, format: format)
            engine.prepare()
            try engine.start()
        } catch {
            engine.stop()
            try? AVAudioSession.sharedInstance().setActive(
                false,
                options: .notifyOthersOnDeactivation
            )
            print("Unable to start audio: \(error)")
            return nil
        }
    }

    func enqueue(from core: EmulatorCore) {
        let samples = core.drainAudio(maxSamples: maximumSamplesPerFrame)
        let frameCount = samples.count / channelCount
        guard frameCount > 0,
              let buffer = AVAudioPCMBuffer(
                  pcmFormat: format,
                  frameCapacity: AVAudioFrameCount(frameCount)
              ),
              let channels = buffer.floatChannelData
        else {
            return
        }

        buffer.frameLength = AVAudioFrameCount(frameCount)
        for frame in 0..<frameCount {
            for channel in 0..<channelCount {
                channels[channel][frame] = Float(
                    samples[frame * channelCount + channel]
                ) / Self.int16FullScale
            }
        }

        playerNode.scheduleBuffer(buffer)
        if !playerNode.isPlaying {
            playerNode.play()
        }
    }

    func stop() {
        playerNode.stop()
        engine.stop()
        try? AVAudioSession.sharedInstance().setActive(
            false,
            options: .notifyOthersOnDeactivation
        )
    }
}
