import assert from "node:assert/strict";
import test from "node:test";
import { greeting } from "../src/greeting";

test("greeting returns message", () => {
  assert.equal(greeting("Ada"), "Hello, Ada");
});
