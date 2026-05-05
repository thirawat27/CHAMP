/// <reference types="@testing-library/jest-dom" />

import { expect, afterEach, vi } from 'vitest';
import { cleanup } from '@testing-library/react';
import * as matchers from '@testing-library/jest-dom/matchers';

// Extend Vitest's expect with jest-dom matchers
expect.extend(matchers);

// Cleanup after each test
afterEach(() => {
  cleanup();
});

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/shell', () => ({
  open: vi.fn(),
}));

// Mock window.location
Object.defineProperty(window, 'location', {
  value: {
    href: 'http://localhost:1420/',
    origin: 'http://localhost:1420',
    protocol: 'http:',
    host: 'localhost:1420',
    hostname: 'localhost',
    port: '1420',
    pathname: '/',
    search: {},
    hash: {},
  },
  writable: true,
});
