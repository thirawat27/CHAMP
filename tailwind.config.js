/** @type {import('tailwindcss').Config} */
// Tailwind v4 uses CSS-first configuration via @import "tailwindcss" and @theme blocks
// This minimal config is kept for content path scanning
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  // Theme is now defined in src/App.css using @theme blocks
  // This config is minimal for v4 compatibility
}
