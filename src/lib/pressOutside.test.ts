import { expect, test } from "bun:test";
import { listenForPressOutside } from "./pressOutside";

function clickAnotherField(page: EventTarget) {
  page.dispatchEvent(new Event("mousedown"));
  page.dispatchEvent(new Event("click"));
}

test("switching fields stops the old capture before the new one starts", () => {
  const page = new EventTarget();
  const log: string[] = [];
  // The clicked field starts recording from its click handler, which React runs
  // before listeners on the document.
  page.addEventListener("click", () => log.push("start new"));
  listenForPressOutside(page, () => false, () => log.push("stop old"));

  clickAnotherField(page);
  expect(log).toEqual(["stop old", "start new"]);
});

test("a press inside the field and a removed listener keep the capture", () => {
  const page = new EventTarget();
  let stops = 0;
  const unsubscribe = listenForPressOutside(page, () => true, () => stops++);
  clickAnotherField(page);
  expect(stops).toBe(0);

  const removed = listenForPressOutside(page, () => false, () => stops++);
  removed();
  clickAnotherField(page);
  expect(stops).toBe(0);
  unsubscribe();
});
