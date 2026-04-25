import "server-only";

export type ChainlinkEnv = {
  baseUrl: string;
  userId: string;
  apiKey: string;
};

/**
 * Server-only. Throws if any required var is missing (BFF will map to 503/500 in routes).
 */
export function getChainlinkConfig(): ChainlinkEnv {
  const baseUrl = process.env.CHAINLINK_BASE_URL;
  const userId = process.env.CHAINLINK_USER_ID;
  const apiKey = process.env.CHAINLINK_API_KEY;
  if (!baseUrl || !userId || !apiKey) {
    throw new Error(
      "Missing server env: set CHAINLINK_BASE_URL, CHAINLINK_USER_ID, CHAINLINK_API_KEY in .env.local"
    );
  }
  return {
    baseUrl: baseUrl.replace(/\/$/, ""),
    userId,
    apiKey,
  };
}

export function isChainlinkConfigured(): boolean {
  return Boolean(
    process.env.CHAINLINK_BASE_URL &&
      process.env.CHAINLINK_USER_ID &&
      process.env.CHAINLINK_API_KEY
  );
}
