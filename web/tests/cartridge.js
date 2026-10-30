export async function selectDemo(page) {
  await page.locator('#change-cartridge').click();
  await page.locator('#demo').click();
}
