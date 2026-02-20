import puppeteer from 'puppeteer';

(async () => {
    const browser = await puppeteer.launch({
        args: ['--no-sandbox', '--disable-setuid-sandbox'],
        defaultViewport: {
            width: 1280,
            height: 800,
            deviceScaleFactor: 1,
        }
    });
    const page = await browser.newPage();

    // Settings
    await page.goto('http://localhost:1420/#/settings', { waitUntil: 'networkidle0' });
    await new Promise(r => setTimeout(r, 1000));
    await page.screenshot({ path: '/home/xiaobai/.gemini/antigravity/brain/6d618f96-202f-40cf-8ee3-24b7bb544082/screen_settings_tech.png' });
    console.log('Settings saved');

    // Websites
    await page.goto('http://localhost:1420/#/websites', { waitUntil: 'networkidle0' });
    await new Promise(r => setTimeout(r, 1000));
    await page.screenshot({ path: '/home/xiaobai/.gemini/antigravity/brain/6d618f96-202f-40cf-8ee3-24b7bb544082/screen_websites_tech.png' });
    console.log('Websites saved');

    // Documents
    await page.goto('http://localhost:1420/#/documents', { waitUntil: 'networkidle0' });
    await new Promise(r => setTimeout(r, 1000));
    await page.screenshot({ path: '/home/xiaobai/.gemini/antigravity/brain/6d618f96-202f-40cf-8ee3-24b7bb544082/screen_documents_tech.png' });
    console.log('Documents saved');

    await browser.close();
})();
