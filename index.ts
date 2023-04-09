import puppeteer, {
  Browser,
  ElementHandle,
} from "https://deno.land/x/puppeteer@16.2.0/mod.ts";

const MANIFEST_URL =
  "https://www.chinacdc.cn/jkzt/crb/zl/szkb_11803/jszl_13141/";

const startBrowser = async () => {
  const browser = await puppeteer.launch({
    headless: false,
    defaultViewport: { width: 1280, height: 768 },
    args: ["--window-size=1280,768", "--no-sandbox"],
  });
  return browser;
};

interface PostInfo {
  title: string;
  date: string;
  link: string;
}

const getPostList = async (browser: Browser): Promise<PostInfo[]> => {
  const page = await browser.newPage();
  await page.goto(MANIFEST_URL, { waitUntil: "load" });
  const firstItem = await page.$(
    ".main .cn-main .cn-main-right .item-top .item-top-text",
  );
  const remainItems = await page.$$(
    ".main .cn-main .cn-main-right .item-top .item-bottom ul li",
  );
  if (!firstItem) {
    throw new Error("no first item");
  }
  const links = await Promise.all(
    [firstItem, ...remainItems].map(async (el) => {
      const anchor = assert(await el.$("a"));
      const link = "" + assert(await getElementProperty(anchor, "href"));
      const title = (await getElementTextContent(anchor)).trim();
      const originalDate =
        (await getElementTextContent(assert(await el.$("a + span"))))
          .trim();
      const date = assert(originalDate.match(/\d{4}-\d{2}-\d{2}/))[0];
      return { title, date, link };
    }),
  );
  await page.close();
  return links;
};

const getPostData = async (browser: Browser, post: PostInfo) => {
  const page = await browser.newPage();
  await page.goto(post.link, { waitUntil: "load" });
  const paragraphs = await Promise.all(
    (await page.$$(".TRS_Editor .TRS_Editor p")).map(
      async (p) => {
        return (await getElementTextContent(p)).replace(/\s+/g, "");
      },
    ),
  );
  try {
    for (const p of paragraphs) {
      const flag = "检测阳性率";
      const indexOfFlag = p.indexOf(flag);
      if (indexOfFlag !== -1) {
        const reDateAndCount =
          /((?:\d{4}年)?\d+月\d+日).{1,6}?((?:\d+\.)?\d+(?:万)?)/;
        const beforeText = p.slice(0, indexOfFlag);
        const { date: countDate, count: countString } = assert(
          beforeText.split(/，|；|。/).reverse().map((seg) => {
            const match = seg.match(reDateAndCount);
            return match && { date: match[1], count: match[2] };
          }).find(Boolean),
        );
        const count = parseNumber(countString);
        if (isNaN(count)) {
          throw new Error("invalid count: " + countString);
        }

        const afterText = p.slice(indexOfFlag + flag.length);
        const rePositivePercent =
          /((?:\d{4}年)?\d+月\d+日).{1,4}?((?:\d+\.)?(\d+)%)/;
        const { date: positivePercentDate, positivePercent } = assert(
          afterText.split(/，|；|。/).reverse().map((seg) => {
            const match = seg.match(rePositivePercent);
            return match &&
              { date: match[1], positivePercent: match[2] };
          }).find(Boolean),
        );

        return {
          count,
          countDate,
          positivePercent,
          positivePercentDate,
        };
      }
    }
    console.warn("no count found for " + post.date + ": " + post.link);
  } finally {
    await page.close();
  }
};

const parseNumber = (num: string): number => {
  if (num.endsWith("万")) {
    return +num.slice(0, num.length - 1) * 10000;
  }
  return +num;
};

const assert = <T>(value: T | undefined | null): T => {
  if (value == null) {
    throw new Error("null");
  }
  return value;
};

const getElementTextContent = async (el: ElementHandle<any>) => {
  return "" + assert(
    await (await el.getProperty(
      "textContent",
    )).jsonValue(),
  );
};

const getElementProperty = async (el: ElementHandle<any>, prop: string) => {
  return await (await el.getProperty(
    prop,
  )).jsonValue();
};

const browser = await startBrowser();
const postList = await getPostList(browser);
const dataList = [];
for (const post of postList) {
  const data = await getPostData(browser, post);
  dataList.push({ data, post });
}
await browser.close();

const bytes = new TextEncoder().encode(JSON.stringify(dataList, null, "  "));
await Deno.writeFile("stats.json", bytes);
