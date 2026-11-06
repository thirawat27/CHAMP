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

import { usePreferencesStore } from "../stores/preferencesStore";

// Audio context singleton
let audioContext: AudioContext | null = null;
let suspendTimer: ReturnType<typeof setTimeout> | undefined;

/** Longer than the longest note, so the audio thread sleeps while CHAMP sits in the tray. */
const SUSPEND_AFTER_IDLE_MS = 5000;

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
  const context = audioContext;
  clearTimeout(suspendTimer);
  suspendTimer = setTimeout(() => {
    context.suspend().catch(console.error);
  }, SUSPEND_AFTER_IDLE_MS);
  return context;
}

function isSoundEnabled(): boolean {
  return usePreferencesStore.getState().soundEnabled;
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
 * STOP – polite power down
 * A4 → E4 → C4  (440 / 329 / 261 Hz)
 * Feels like: "safely disengaged", standby
 */
function playStopSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  const now = ctx.currentTime;

  const phrase = [
    { freq: 440.0, t: 0.0 }, // A4
    { freq: 329.63, t: 0.18 }, // E4
    { freq: 261.63, t: 0.36 }, // C4
  ];

  phrase.forEach(({ freq, t }) => playNote(ctx, freq, now + t, 0.08, 1.4, 0.15));
}

/**
 * SUCCESS / COMPLETE – satisfying confirmation
 * C4 → E4 → G4 → C5  (261 / 329 / 392 / 523 Hz)
 * Feels like: "action successful", positive reinforcement
 */
function playCompleteSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  const now = ctx.currentTime;

  const phrase = [
    { freq: 261.63, t: 0.0 }, // C4
    { freq: 329.63, t: 0.12 }, // E4
    { freq: 392.0, t: 0.24 }, // G4
    { freq: 523.25, t: 0.36 }, // C5 – final note longer resonance
  ];

  phrase.forEach(({ freq, t }, i) => playNote(ctx, freq, now + t, 0.05, i === 3 ? 1.8 : 0.8, 0.18));
}

/**
 * ERROR – soft warning (non-intrusive minor second)
 * E5 → D#5  (659 / 622 Hz)
 * Feels like: "attention needed", polite alert
 */
function playErrorSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  const now = ctx.currentTime;

  const phrase = [
    { freq: 659.25, t: 0.0 }, // E5
    { freq: 622.25, t: 0.2 }, // D#5
  ];

  phrase.forEach(({ freq, t }) => playNote(ctx, freq, now + t, 0.06, 1.0, 0.16));
}

// ---------------------------------------------------------------------------
// UI sounds
// ---------------------------------------------------------------------------

/**
 * CLICK – clean short tap
 */
function playClickSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  playNote(ctx, 783.99, ctx.currentTime, 0.02, 0.2, 0.12); // G5
}

/**
 * TOGGLE – two-note quick confirmation
 */
function playToggleSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  const now = ctx.currentTime;
  playNote(ctx, 880.0, now, 0.02, 0.15, 0.1); // A5
  playNote(ctx, 659.25, now + 0.1, 0.02, 0.2, 0.08); // E5
}

// ---------------------------------------------------------------------------
// Notification tone – called when a notice banner appears
// ---------------------------------------------------------------------------

/**
 * Play the appropriate sound when a status notification fires.
 *
 * tone    action   sound
 * ------  -------  -----
 * success start    playCompleteSound
 * success restart  playCompleteSound
 * success stop     playStopSound
 * error   *        playErrorSound
 */
function playNotificationSound(
  tone: "success" | "error",
  action?: "start" | "restart" | "stop"
): void {
  if (tone === "error") {
    playErrorSound();
  } else if (action === "stop") {
    playStopSound();
  } else {
    playCompleteSound();
  }
}

// ---------------------------------------------------------------------------
// AudioManager object
// ---------------------------------------------------------------------------

export const AudioManager = {
  playClick: playClickSound,
  playToggle: playToggleSound,
  playNotification: playNotificationSound,

  // Browsers only allow audio after a user gesture, so unlock it on the first click.
  initialize: () => {
    if (isSoundEnabled()) getAudioContext();
  },
};
