/**
 * Audio Manager - Modern Pentatonic / Zen Tech (EV Style)
 *
 * Clean, crystalline, soft and calming sound effects using Web Audio API.
 * Inspired by modern Electric Vehicle (e.g. BYD) interface chimes and alerts.
 *
 * Design principles:
 * - Pure sine waves for a smooth, futuristic, and unobtrusive tone
 * - Higher registers (400Hz - 900Hz) for clarity without harshness
 * - Fast attack for responsiveness, followed by a soft, natural tail
 * - Octave overtones to simulate a digital glass/bell resonance
 * - Pentatonic scale (C, D, E, G, A) to ensure all tones sound harmonious and "Zen"
 */

import { useLanguageStore } from "../stores/languageStore";

// Audio context singleton
let audioContext: AudioContext | null = null;

function getAudioContext(): AudioContext | null {
  if (typeof window === "undefined") return null;
  if (!audioContext) {
    try {
      audioContext = new (
        window.AudioContext ||
        (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext
      )();
    } catch {
      console.warn("Web Audio API not supported");
      return null;
    }
  }
  if (audioContext.state === "suspended") {
    audioContext.resume().catch(console.error);
  }
  return audioContext;
}

function isSoundEnabled(): boolean {
  return useLanguageStore.getState().soundEnabled;
}

// ---------------------------------------------------------------------------
// Shared helper – plays one clear EV chime note (pure sine + octave overtone)
// ---------------------------------------------------------------------------
function playNote(
  ctx: AudioContext,
  freq: number,
  startTime: number,
  attack: number,
  decay: number,
  gain: number
): void {
  // Main chime – pure sine for that clean EV sound
  const sine = ctx.createOscillator();
  const sineGain = ctx.createGain();

  sine.type = "sine";
  sine.frequency.setValueAtTime(freq, startTime);

  sine.connect(sineGain);
  sineGain.connect(ctx.destination);

  sineGain.gain.setValueAtTime(0, startTime);
  sineGain.gain.linearRampToValueAtTime(gain * 0.8, startTime + attack);
  sineGain.gain.exponentialRampToValueAtTime(0.0001, startTime + attack + decay);

  sine.start(startTime);
  sine.stop(startTime + attack + decay + 0.05);

  // Digital bell overtone – sine at 2x frequency, very short decay for a "glassy" strike
  const overtone = ctx.createOscillator();
  const overtoneGain = ctx.createGain();

  overtone.type = "sine";
  overtone.frequency.setValueAtTime(freq * 2, startTime); // One octave up

  overtone.connect(overtoneGain);
  overtoneGain.connect(ctx.destination);

  overtoneGain.gain.setValueAtTime(0, startTime);
  overtoneGain.gain.linearRampToValueAtTime(gain * 0.2, startTime + attack * 0.5);
  overtoneGain.gain.exponentialRampToValueAtTime(0.0001, startTime + attack + decay * 0.3);

  overtone.start(startTime);
  overtone.stop(startTime + attack + decay * 0.3 + 0.05);
}

// ---------------------------------------------------------------------------
// Service sounds (Pentatonic EV Chimes)
// ---------------------------------------------------------------------------

/**
 * START – uplifting crystalline phrase
 * C5 → E5 → A5  (523 / 659 / 880 Hz)
 * Feels like: "system online", modern, airy
 */
export function playStartSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  const now = ctx.currentTime;

  const phrase = [
    { freq: 523.25, t: 0.00 },  // C5
    { freq: 659.25, t: 0.15 },  // E5
    { freq: 880.00, t: 0.30 },  // A5
  ];

  phrase.forEach(({ freq, t }) =>
    playNote(ctx, freq, now + t, 0.05, 1.2, 0.18)
  );
}

/**
 * RESTART – gentle cyclical refresh
 * E5 → C5 → E5  (659 / 523 / 659 Hz)
 * Feels like: "recalibrating", soft reset
 */
export function playRestartSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  const now = ctx.currentTime;

  const phrase = [
    { freq: 659.25, t: 0.00 },  // E5
    { freq: 523.25, t: 0.15 },  // C5
    { freq: 659.25, t: 0.30 },  // E5
  ];

  phrase.forEach(({ freq, t }) =>
    playNote(ctx, freq, now + t, 0.05, 1.0, 0.16)
  );
}

/**
 * STOP – polite power down
 * A4 → E4 → C4  (440 / 329 / 261 Hz)
 * Feels like: "safely disengaged", standby
 */
export function playStopSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  const now = ctx.currentTime;

  const phrase = [
    { freq: 440.00, t: 0.00 },  // A4
    { freq: 329.63, t: 0.18 },  // E4
    { freq: 261.63, t: 0.36 },  // C4
  ];

  phrase.forEach(({ freq, t }) =>
    playNote(ctx, freq, now + t, 0.08, 1.4, 0.15)
  );
}

/**
 * SUCCESS / COMPLETE – satisfying confirmation
 * C4 → E4 → G4 → C5  (261 / 329 / 392 / 523 Hz)
 * Feels like: "action successful", positive reinforcement
 */
export function playCompleteSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  const now = ctx.currentTime;

  const phrase = [
    { freq: 261.63, t: 0.00 },  // C4
    { freq: 329.63, t: 0.12 },  // E4
    { freq: 392.00, t: 0.24 },  // G4
    { freq: 523.25, t: 0.36 },  // C5 – final note longer resonance
  ];

  phrase.forEach(({ freq, t }, i) =>
    playNote(ctx, freq, now + t, 0.05, i === 3 ? 1.8 : 0.8, 0.18)
  );
}

/**
 * ERROR – soft warning (non-intrusive minor second)
 * E5 → D#5  (659 / 622 Hz)
 * Feels like: "attention needed", polite alert
 */
export function playErrorSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  const now = ctx.currentTime;

  const phrase = [
    { freq: 659.25, t: 0.00 },  // E5
    { freq: 622.25, t: 0.20 },  // D#5
  ];

  phrase.forEach(({ freq, t }) =>
    playNote(ctx, freq, now + t, 0.06, 1.0, 0.16)
  );
}

// ---------------------------------------------------------------------------
// UI sounds
// ---------------------------------------------------------------------------

/**
 * CLICK – clean short tap
 */
export function playClickSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  playNote(ctx, 783.99, ctx.currentTime, 0.02, 0.2, 0.12); // G5
}

/**
 * TOGGLE – two-note quick confirmation
 */
export function playToggleSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  const now = ctx.currentTime;
  playNote(ctx, 880.00, now,        0.02, 0.15, 0.10); // A5
  playNote(ctx, 659.25, now + 0.10, 0.02, 0.20, 0.08); // E5
}

/**
 * HOVER – silent (no sound on hover per UX decision)
 */
export function playHoverSound(): void {
  // intentionally silent
}

// ---------------------------------------------------------------------------
// Loading / progress sounds
// ---------------------------------------------------------------------------

/**
 * LOADING PULSE – gentle recurring chime
 */
export function playLoadingPulseSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  playNote(ctx, 329.63, ctx.currentTime, 0.05, 0.6, 0.10); // E4
}

/**
 * LOADING STREAM – very subtle sweeping confirmation tone
 */
export function playLoadingStreamSound(progress: number): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  // G4 (392 Hz) rising gently to C5 (523 Hz)
  const freq = 392.00 + (progress / 100) * (523.25 - 392.00);
  playNote(ctx, freq, ctx.currentTime, 0.03, 0.15, 0.06);
}

// ---------------------------------------------------------------------------
// Notification tone – called when a notice banner appears
// ---------------------------------------------------------------------------

/**
 * Play the appropriate sound when a status notification fires.
 *
 * tone    action   sound
 * ------  -------  -----
 * info    start    playStartSound  (pending start)
 * info    restart  playRestartSound
 * info    stop     (silent – don't interrupt)
 * success start    playCompleteSound
 * success restart  playCompleteSound
 * success stop     playStopSound
 * error   *        playErrorSound
 */
export function playNotificationSound(
  tone: "info" | "success" | "error",
  action?: "start" | "restart" | "stop"
): void {
  if (tone === "error") {
    playErrorSound();
    return;
  }
  if (tone === "success") {
    if (action === "stop") {
      playStopSound();
    } else {
      playCompleteSound();
    }
    return;
  }
  // info
  if (action === "start")   { playStartSound();   return; }
  if (action === "restart") { playRestartSound();  return; }
  // info + stop → silent (user already heard the stop click)
}

// ---------------------------------------------------------------------------
// AudioManager object
// ---------------------------------------------------------------------------

export const AudioManager = {
  // Service lifecycle
  playStart:    playStartSound,
  playRestart:  playRestartSound,
  playStop:     playStopSound,
  playComplete: playCompleteSound,

  // Loading / wizard
  playLoadingPulse:  playLoadingPulseSound,
  playLoadingStream: playLoadingStreamSound,

  // UI interactions
  playClick:  playClickSound,
  playToggle: playToggleSound,
  playHover:  playHoverSound,

  // Notification banner
  playNotification: playNotificationSound,

  // Legacy aliases
  playSuccess: playCompleteSound,
  playError:   playErrorSound,

  isSupported: () =>
    typeof window !== "undefined" &&
    !!(window.AudioContext ||
      (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext),

  initialize: getAudioContext,
};

export default AudioManager;

