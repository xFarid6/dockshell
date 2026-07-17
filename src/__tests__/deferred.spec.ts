// Deferred-feature test surface for the frontend. Each todo names the GitHub
// issue that will implement it — mirrors src-tauri/tests/deferred.rs.
import { describe, test } from "vitest";

describe("deferred features", () => {
  test.todo("opens an exec terminal inside a container (issue #2)");
  test.todo("image list/pull/remove view (issue #3)");
  test.todo("volume management view (issue #4)");
  test.todo("network management view (issue #5)");
  test.todo("compose file up/down (issue #6)");
  test.todo("TLS client-cert fields on the connection form (issue #7)");
  test.todo("container detail/inspect view (issue #8)");
  test.todo("live container state via Docker events (issue #10)");
});
