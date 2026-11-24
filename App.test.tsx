// The Rust root tested from TypeScript: `nsh test` runs this file in a
// headless runner it builds with the crate, and `open(".")` starts the root
// (`nsh.json` "root").
import { expect, open, test } from "nsh/test";

test("unlocks with a master password and lists logins", async () => {
  const app = await open(".");
  expect(app.texts()).toContain("Pandex");
  expect(app.texts()).not.toContain("GitHub");

  // Empty password does not unlock.
  await app.press("Unlock");
  expect(app.texts()).not.toContain("Vault");

  await app.type("id:master-password", "correct horse");
  expect(app.value("id:master-password")).toBe("correct horse");
  await app.press("Unlock");
  expect(app.texts()).toContain("Vault");
  expect(app.texts()).toContain("GitHub");
  expect(app.texts()).toContain("Proton Mail");

  await app.press("Lock");
  expect(app.texts()).toContain("Pandex");
  expect(app.texts()).not.toContain("GitHub");
  expect(app.value("id:master-password")).toBe("");
});

test("return key submits the master password", async () => {
  const app = await open(".");
  await app.type("id:master-password", "correct horse");
  await app.submit("id:master-password");
  expect(app.texts()).toContain("Vault");
});
