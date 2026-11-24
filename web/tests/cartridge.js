export async function selectDemo(page, id = 'demo') {
  await page.locator('#change-cartridge').click();
  await page.locator(`#${id}`).click();
}

export async function delayLocalFileRead(page) {
  await page.evaluate(() => {
    const read = File.prototype.arrayBuffer;
    File.prototype.arrayBuffer = async function () {
      await new Promise(resolve => { window.releaseLocalFileRead = resolve; });
      return read.call(this);
    };
  });
  return () => page.evaluate(() => releaseLocalFileRead());
}
