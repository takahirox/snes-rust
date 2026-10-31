export async function selectDemo(page, id = 'demo') {
  await page.locator('#change-cartridge').click();
  await page.locator(`#${id}`).click();
}
