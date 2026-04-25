import { z } from "zod";

/** Shared validation for BFF query parameters. */
export const historyQuerySchema = z.object({
  symbol: z.string().min(3).max(32),
  resolution: z
    .string()
    .min(1)
    .max(8)
    .regex(/^\d+(m|h|d|w|M|y)$/, "Invalid resolution format"),
  from: z.coerce.number().int().positive(),
  to: z.coerce.number().int().positive(),
});

export type HistoryQuery = z.infer<typeof historyQuerySchema>;
