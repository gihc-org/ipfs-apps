import { defineConfig, devices } from '@playwright/test';

const BASE_URL = process.env.BASE_URL ?? 'http://localhost:8888';

export default defineConfig({
  testDir: './tests',
  fullyParallel: false,
  retries: 0,
  reporter: 'list',
  use: {
    baseURL: BASE_URL,
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        permissions: ['microphone'],
        launchOptions: {
          args: [
            '--use-fake-ui-for-media-stream',
            '--use-fake-device-for-media-stream',
            // Headless betyder kun "intet vindue" — uden --mute-audio spiller
            // den falske mikrofon (en tone) ud af maskinens højttalere.
            '--mute-audio',
          ],
        },
      },
    },
    // Firefox er brugerens primære browser på alle enheder, men binæren skal
    // hentes først (`npx playwright install firefox`), og den skal derfor ikke
    // tvinge CI til at downloade endnu en browser. Slåes til med PW_FIREFOX=1
    // (se npm run test:test:firefox).
    ...(process.env.PW_FIREFOX
      ? [
          {
            name: 'firefox',
            use: {
              ...devices['Desktop Firefox'],
              launchOptions: {
                firefoxUserPrefs: {
                  // Samme falske mediestrømme som Chromium får via args.
                  'media.navigator.streams.fake': true,
                  'media.navigator.permission.disabled': true,
                  'media.autoplay.default': 0,
                  // ... og samme dæmpning: headless Firefox spiller ellers
                  // testlyden ud af maskinens højttalere.
                  'media.volume_scale': '0.0',
                },
              },
            },
          },
        ]
      : []),
  ],
  webServer: process.env.BASE_URL ? undefined : {
    command: 'python3 -m http.server 8888 --directory ../frontend',
    url: BASE_URL,
    reuseExistingServer: true,
  },
});
