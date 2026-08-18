import { expect, test } from "bun:test";
import { findEmbeddedCredential } from "../scripts/verify-dist.mjs";

test("distribution scan permits credential labels and rejects high-confidence literals", () => {
  expect(findEmbeddedCredential("Enter an API key or provider credential.")).toBeNull();
  expect(findEmbeddedCredential('authorization: `Bearer ${credential}`')).toBeNull();
  const syntheticOpenAiShape = `sk-${"A".repeat(40)}`;
  expect(findEmbeddedCredential(`const credential = "${syntheticOpenAiShape}";`)).toContain("OpenAI-style");
  const syntheticGithubShape = `ghp_${"B".repeat(36)}`;
  expect(findEmbeddedCredential(syntheticGithubShape)).toContain("GitHub-style");
});
