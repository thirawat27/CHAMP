# CAMPP Automated Test Suite

## Overview

This directory contains automated tests for the CAMPP Phase 3 (Process Manager) UI and backend functionality.

## Test Structure

```
campp/
├── src/
│   ├── components/
│   │   ├── ServiceCard.test.tsx      # ServiceCard component tests
│   │   └── Dashboard.test.tsx         # Dashboard component tests
│   └── test/
│       ├── setup.ts                   # Vitest setup with mocks
│       ├── mocks/
│       │   └── serviceMocks.ts         # Mock data and utilities
│       └── utils/
│           └── testHelpers.tsx         # Test helper functions
├── src-tauri/
│   ├── tests/
│   │   └── ui_commands.rs             # Rust integration tests
│   └── src/
│       └── process/
│           ├── mod.rs                 # Contains unit tests
│           └── manager.rs             # Contains integration tests
```

## Running Tests

### Frontend Tests (Vitest)

```bash
# Run all frontend tests
npm test

# Run tests in watch mode
npm run test

# Run tests with UI
npm run test:ui

# Run tests once
npm run test:run

# Run tests with coverage
npm run test:run -- --coverage
```

### Backend Tests (Rust)

```bash
# Run all Rust tests
cd src-tauri
cargo test

# Run tests with output
cargo test -- --nocapture

# Run only unit tests (fast, no binaries required)
cargo test --lib

# Run integration tests (requires runtime binaries)
cargo test --lib -- --ignored

# Run specific test
cargo test test_get_all_statuses

# Run specific test module
cargo test process::tests
```

### Run All Tests

```bash
# Run both frontend and backend tests
npm run test:run && cd src-tauri && cargo test
```

## Test Cases

### Frontend Test Cases (Phase 3 UI)

| Test ID | Description | File |
|---------|-------------|------|
| TC-PM-UI-01 | ServiceCard rendering | ServiceCard.test.tsx |
| TC-PM-UI-02 | Stopped state display | ServiceCard.test.tsx |
| TC-PM-UI-03 | Running state display | ServiceCard.test.tsx |
| TC-PM-UI-04 | Starting state display | ServiceCard.test.tsx |
| TC-PM-UI-05 | Stopping state display | ServiceCard.test.tsx |
| TC-PM-UI-06 | Error state display | ServiceCard.test.tsx |
| TC-PM-UI-07 | All service types | ServiceCard.test.tsx |
| TC-PM-DASH-01 | Dashboard initial display | Dashboard.test.tsx |
| TC-PM-DASH-02 | Status refresh | Dashboard.test.tsx |
| TC-PM-DASH-03 | Start service | Dashboard.test.tsx |
| TC-PM-DASH-04 | Stop service | Dashboard.test.tsx |
| TC-PM-DASH-05 | Restart service | Dashboard.test.tsx |
| TC-PM-DASH-06 | Error handling | Dashboard.test.tsx |
| TC-PM-DASH-07 | Quick actions | Dashboard.test.tsx |
| TC-PM-DASH-08 | All services running | Dashboard.test.tsx |

### Backend Test Cases (Rust)

| Test ID | Description | File |
|---------|-------------|------|
| TC-PM-RS-01 | Get all statuses | ui_commands.rs |
| TC-PM-RS-02 | Initial state is stopped | ui_commands.rs |
| TC-PM-RS-03 | Display names | ui_commands.rs |
| TC-PM-RS-04 | Service ports | ui_commands.rs |
| TC-PM-RS-05 | Service descriptions | ui_commands.rs |
| TC-PM-RS-06 | State serialization | ui_commands.rs |
| TC-PM-RS-07 | Type serialization | ui_commands.rs |
| TC-PM-RS-08 | Independent states | ui_commands.rs |
| TC-PM-RS-09 | Error messages | ui_commands.rs |
| TC-PM-RS-10 | Stop stopped service | ui_commands.rs |

## Test Coverage

### Current Coverage

- **Frontend**: Component rendering, state management, user interactions
- **Backend**: Service status retrieval, serialization, state management

### Coverage Goals

- **Components**: > 80% coverage
- **Commands**: > 90% coverage
- **Process Manager**: > 85% coverage

## Mock Data

### Mock Service Map

```typescript
const mockServiceMap: ServiceMap = {
  Caddy: {
    name: 'Caddy',
    displayName: 'Caddy',
    description: 'Web server',
    port: 8080,
    state: 'Stopped',
    error_message: null,
  },
  PhpFpm: {
    name: 'PhpFpm',
    displayName: 'PHP-FPM',
    description: 'PHP runtime',
    port: 9000,
    state: 'Stopped',
    error_message: null,
  },
  MariaDB: {
    name: 'MariaDB',
    displayName: 'MariaDB',
    description: 'Database',
    port: 3307,
    state: 'Stopped',
    error_message: null,
  },
};
```

## Continuous Integration

### GitHub Actions Workflow

```yaml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [windows-latest, macos-latest, ubuntu-latest]
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          profile: minimal
          toolchain: stable
      - name: Install Node.js
        uses: actions/setup-node@v3
        with:
          node-version: '20'
      - name: Install dependencies
        run: npm install
      - name: Run frontend tests
        run: npm run test:run
      - name: Run backend tests
        run: cd src-tauri && cargo test
```

## Test Data Attributes

Components use `data-testid` attributes for reliable testing:

```tsx
<div data-testid="service-card-Caddy">
  <span data-testid="service-state-Caddy">Stopped</span>
  <button data-testid="start-button-Caddy">Start</button>
  <button data-testid="stop-button-Caddy">Stop</button>
  <button data-testid="restart-button-Caddy">Restart</button>
</div>
```

## Debugging Tests

### Frontend Tests

```bash
# Run with verbose output
npm run test:run -- --reporter=verbose

# Run specific test file
npm run test:run -- ServiceCard

# Run with UI for interactive debugging
npm run test:ui
```

### Backend Tests

```bash
# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_get_all_statuses -- --nocapture

# Run tests in debug mode
cargo test -- --exact --show-output
```

## Troubleshooting

### Common Issues

1. **"Cannot find module" errors**
   - Run `npm install` to ensure all dependencies are installed
   - Check that test files have correct `.test.tsx` or `.test.ts` extension

2. **"Tauri invoke not mocked" errors**
   - Ensure `src/test/setup.ts` is loaded by Vitest
   - Check that `@tauri-apps/api/core` is properly mocked

3. **Rust tests fail with "binary not found"**
   - Integration tests require runtime binaries
   - Run `npm run tauri dev` and complete the download wizard first
   - Or run only unit tests with `cargo test --lib` (skips `--ignored` tests)

## Writing New Tests

### Frontend Test Template

```tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import YourComponent from './YourComponent';

describe('YourComponent', () => {
  it('should do something', () => {
    render(<YourComponent />);
    expect(screen.getByText('Expected Text')).toBeInTheDocument();
  });
});
```

### Backend Test Template

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        let result = function_to_test();
        assert_eq!(result, expected_value);
    }
}
```

## Next Steps

1. Add E2E tests with Playwright for full user flows
2. Add performance tests for service startup times
3. Add accessibility tests with axe-core
4. Add visual regression tests

---

For more information, see:
- [Vitest Documentation](https://vitest.dev/)
- [React Testing Library](https://testing-library.com/react)
- [Tauri Testing Guide](https://tauri.app/v1/guides/testing/introduction/)
