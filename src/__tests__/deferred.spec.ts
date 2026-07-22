// Deferred-feature test surface for the frontend. Each todo names the GitHub
// issue that will implement it — mirrors src-tauri/tests/deferred.rs.
import { describe, test } from "vitest";

describe("deferred features", () => {
  test.todo("volume management view (issue #4)");
  test.todo("network management view (issue #5)");
  test.todo("compose file up/down (issue #6)");
  test.todo("TLS client-cert fields on the connection form (issue #7)");
});
