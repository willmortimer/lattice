# Voice Dictation

Native capture uses AVAudioEngine with a 300 ms pre-roll ring. FinalizationMode
defaults to StreamingFlush; IndependentOfflineRedecode stays deferred until
voice-eval gates pass.

Say identifiers like `VoiceContextBuilder` or paths like `Inbox/Sample capture`
so glossary and ITN normalize before editor commit.
