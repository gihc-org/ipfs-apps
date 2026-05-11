import { test, expect, Browser, Page } from '@playwright/test';
import { register, login, uniqueUser, deleteUser } from './helpers';

async function newSession(browser: Browser, username: string, password: string): Promise<Page> {
  const ctx = await browser.newContext();
  const page = await ctx.newPage();
  await register(page, username, password);
  await login(page, username, password);
  return page;
}

test('start DM med en anden bruger', async ({ browser }) => {
  const pw = 'Password123!';
  const alice = uniqueUser();
  const bob = `bob_${Date.now()}`;

  const pageAlice = await newSession(browser, alice, pw);
  const pageBob = await newSession(browser, bob, pw);

  // Alice søger efter Bob og starter DM
  await pageAlice.fill('#userSearch', bob);
  await pageAlice.locator('.user-card', { hasText: bob }).waitFor();
  await pageAlice.locator('.user-card', { hasText: bob }).locator('button').click();
  await expect(pageAlice).toHaveURL(/chat\.html.*is_dm=true/);

  await deleteUser(pageAlice);
  await deleteUser(pageBob);
});

test('DM vises i listen efter oprettelse', async ({ browser }) => {
  const pw = 'Password123!';
  const alice = uniqueUser();
  const bob = `bob_${Date.now()}`;

  const pageAlice = await newSession(browser, alice, pw);
  const pageBob = await newSession(browser, bob, pw);

  await pageAlice.fill('#userSearch', bob);
  await pageAlice.locator('.user-card', { hasText: bob }).waitFor();
  await pageAlice.locator('.user-card', { hasText: bob }).locator('button').click();
  await pageAlice.waitForURL(/chat\.html/);

  // Gå tilbage til rooms og tjek DM-listen
  await pageAlice.goto('/rooms.html');
  await expect(pageAlice.locator('#dmList .dm-card', { hasText: bob })).toBeVisible();

  await deleteUser(pageAlice);
  await deleteUser(pageBob);
});
