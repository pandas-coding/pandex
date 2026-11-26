// The Rust root tested from TypeScript: `nsh test` runs this file in a
// headless runner it builds with the crate, and `open(".")` starts the root
// (`nsh.json` "root"). Headless runs have no data directory, so the app uses
// an in-memory store and every `open(".")` starts at first run.
import { expect, open, test } from "nsh/test";

async function createVault(app, pw = "correct horse") {
  await app.type("id:create-password", pw);
  await app.type("id:create-confirm", pw);
  await app.press("Create vault");
}

test("first run: create vault, add entry, lock/unlock roundtrip", async () => {
  const app = await open(".");
  expect(app.texts()).toContain("Create your vault");
  expect(app.texts()).not.toContain("Vault");

  await createVault(app);
  expect(app.texts()).toContain("Vault");

  // Add an entry; the list shows title/username but never the password.
  await app.press("Add");
  expect(app.texts()).toContain("Add entry");
  await app.type("id:entry-title", "GitHub");
  await app.type("id:entry-username", "octocat@example.com");
  await app.type("id:entry-password", "s3cret");
  await app.press("Save");
  expect(app.texts()).toContain("GitHub");
  expect(app.texts()).toContain("octocat@example.com");
  expect(app.texts()).not.toContain("s3cret");

  // Lock clears the list and the password field.
  await app.press("Lock");
  expect(app.texts()).toContain("Pandex");
  expect(app.texts()).not.toContain("GitHub");
  expect(app.value("id:master-password")).toBe("");

  // Unlock restores the persisted entry.
  await app.type("id:master-password", "correct horse");
  await app.press("Unlock");
  expect(app.texts()).toContain("Vault");
  expect(app.texts()).toContain("GitHub");

  await app.press("Lock");
  expect(app.texts()).not.toContain("GitHub");
});

test("return key submits the create form", async () => {
  const app = await open(".");
  await app.type("id:create-password", "correct horse");
  await app.type("id:create-confirm", "correct horse");
  await app.submit("id:create-confirm");
  expect(app.texts()).toContain("Vault");
});

test("mismatched confirmation is rejected", async () => {
  const app = await open(".");
  await app.type("id:create-password", "correct horse");
  await app.type("id:create-confirm", "different");
  await app.press("Create vault");
  expect(app.texts()).toContain("Passwords do not match.");
  expect(app.texts()).toContain("Create your vault");
  expect(app.texts()).not.toContain("Vault");
});

test("wrong master password is rejected", async () => {
  const app = await open(".");
  await createVault(app);
  await app.press("Lock");
  await app.type("id:master-password", "wrong-password");
  await app.press("Unlock");
  expect(app.texts()).toContain("Wrong password.");
  expect(app.texts()).toContain("Pandex");
  expect(app.texts()).not.toContain("Vault");
});

test("idle timeout auto-locks and wipes the open form", async () => {
  const app = await open(".", { clock: {} });
  await createVault(app);
  await app.press("Add");
  await app.type("id:entry-title", "GitHub");
  await app.type("id:entry-password", "s3cret");

  // 4 idle minutes: under the 5-minute default, still unlocked.
  await app.clock.fastForward(240_000);
  expect(app.texts()).toContain("Add entry");

  // Crossing the boundary auto-locks: the form and its plaintext are gone.
  await app.clock.fastForward(61_000);
  expect(app.texts()).toContain("Unlock your vault");
  expect(app.texts()).not.toContain("Add entry");
  expect(app.value("id:master-password")).toBe("");

  // Unlocking restores the vault with the form closed and empty.
  await app.type("id:master-password", "correct horse");
  await app.press("Unlock");
  expect(app.texts()).toContain("Vault");
  expect(app.texts()).not.toContain("Add entry");
});

test("activity resets the idle timer", async () => {
  const app = await open(".", { clock: {} });
  await createVault(app);
  await app.clock.fastForward(240_000);

  // Opening the form counts as activity: the timer restarts here.
  await app.press("Add");
  await app.clock.fastForward(240_000);
  expect(app.texts()).toContain("Add entry");

  // 301s after the last activity: locked.
  await app.clock.fastForward(61_000);
  expect(app.texts()).toContain("Unlock your vault");
});

test("going to background locks immediately and stays locked", async () => {
  const app = await open(".");
  await createVault(app);
  await app.press("Add");
  await app.type("id:entry-password", "s3cret");

  await app.setLifecycle("background");
  expect(app.texts()).toContain("Unlock your vault");
  expect(app.texts()).not.toContain("Add entry");

  // Returning to the foreground does not unlock.
  await app.setLifecycle("active");
  expect(app.texts()).toContain("Unlock your vault");

  await app.type("id:master-password", "correct horse");
  await app.press("Unlock");
  expect(app.texts()).toContain("Vault");
  expect(app.texts()).not.toContain("Add entry");
});

test("edit and delete an entry", async () => {
  const app = await open(".");
  await createVault(app);
  await app.press("Add");
  await app.type("id:entry-title", "GitHub");
  await app.type("id:entry-username", "octocat");
  await app.type("id:entry-password", "old-pw");
  await app.press("Save");
  expect(app.texts()).toContain("GitHub");

  // Pressing the row opens the edit form, prefilled via on-demand disclosure.
  await app.press("GitHub");
  expect(app.texts()).toContain("Edit entry");
  expect(app.value("id:entry-title")).toBe("GitHub");
  expect(app.value("id:entry-password")).toBe("old-pw");

  await app.clear("id:entry-title");
  await app.type("id:entry-title", "GitHub Enterprise");
  await app.clear("id:entry-password");
  await app.type("id:entry-password", "new-pw");
  await app.press("Save");
  expect(app.texts()).toContain("GitHub Enterprise");
  expect(app.texts()).not.toContain("GitHub");

  // Delete removes the row.
  await app.press("Delete");
  expect(app.texts()).not.toContain("GitHub Enterprise");
  expect(app.texts()).toContain("Vault");
});
