import type { BountyProblemKind } from "./bounty-problem-kinds";

export interface BountyTemplate {
  id: string;
  kind: BountyProblemKind;
  label: string;
  tagline: string;
  title: string;
  briefing: string;
  instance: Record<string, unknown>;
  suggestedBounty: string;
  suggestedMinWorkScore: string;
  suggestedComplexity: "easy" | "medium" | "hard" | "expert";
  suggestedExpirationDays: string;
}

const TSP_US_15_DISTANCES: number[][] = [
  [0, 224, 95, 240, 880, 839, 567, 1617, 1673, 1920, 2525, 2886, 3028, 2834, 1289],
  [224, 0, 319, 465, 1105, 1001, 722, 1828, 1893, 2083, 2708, 3057, 3178, 2933, 1485],
  [95, 319, 0, 145, 785, 783, 521, 1531, 1581, 1858, 2452, 2818, 2970, 2800, 1207],
  [240, 465, 145, 0, 640, 700, 465, 1395, 1438, 1757, 2334, 2707, 2873, 2740, 1093],
  [880, 1105, 785, 640, 0, 695, 705, 849, 827, 1428, 1875, 2281, 2520, 2570, 716],
  [839, 1001, 783, 700, 695, 0, 280, 949, 1111, 1083, 1713, 2056, 2189, 2045, 1406],
  [567, 722, 521, 465, 705, 280, 0, 1179, 1305, 1362, 1991, 2336, 2463, 2281, 1364],
  [1617, 1828, 1531, 1395, 849, 949, 1179, 0, 265, 782, 1044, 1461, 1747, 1982, 1310],
  [1673, 1893, 1581, 1438, 827, 1111, 1305, 265, 0, 1037, 1197, 1618, 1938, 2229, 1141],
  [1920, 2083, 1858, 1757, 1428, 1083, 1362, 782, 1037, 0, 691, 979, 1118, 1203, 2036],
  [2525, 2708, 2452, 2334, 1875, 1713, 1991, 1044, 1197, 691, 0, 421, 770, 1315, 2335],
  [2886, 3057, 2818, 2707, 2281, 2056, 2336, 1461, 1618, 979, 421, 0, 410, 1133, 2756],
  [3028, 3178, 2970, 2873, 2520, 2189, 2463, 1747, 1938, 1118, 770, 410, 0, 802, 3057],
  [2834, 2933, 2800, 2740, 2570, 2045, 2281, 1982, 2229, 1203, 1315, 1133, 802, 0, 3223],
  [1289, 1485, 1207, 1093, 716, 1406, 1364, 1310, 1141, 2036, 2335, 2756, 3057, 3223, 0],
];

export const BOUNTY_TEMPLATES: BountyTemplate[] = [
  {
    id: "subsetsum-starter",
    kind: "SubsetSum",
    label: "Starter",
    tagline: "6 numbers · classroom scale",
    title: "Subset Sum — warehouse pick list",
    briefing: `Find a subset of SKUs whose quantities sum **exactly** to the target pallet weight.

**Solver output:** 0-based indices into \`numbers\` (on-chain \`Solution::SubsetSum\`).

**Verification:** sum of selected numbers equals \`target\`.`,
    instance: { target: 15, numbers: [3, 34, 4, 12, 5, 2] },
    suggestedBounty: "10",
    suggestedMinWorkScore: "150",
    suggestedComplexity: "medium",
    suggestedExpirationDays: "14",
  },
  {
    id: "subsetsum-fleet",
    kind: "SubsetSum",
    label: "Fleet fuel",
    tagline: "12 loads · exact tonnage match",
    title: "Subset Sum — aggregate fuel load matching",
    briefing: `A terminal must pick tanker loads that sum to **exactly** 2,847 barrels. Partial matches do not count.

**Solver output:** index array into \`numbers\`.`,
    instance: {
      target: 2847,
      numbers: [412, 891, 203, 556, 778, 334, 445, 612, 289, 901, 167, 523],
    },
    suggestedBounty: "25",
    suggestedMinWorkScore: "180",
    suggestedComplexity: "hard",
    suggestedExpirationDays: "30",
  },
  {
    id: "tsp-starter",
    kind: "TSP",
    label: "Starter",
    tagline: "5 cities · symmetric matrix",
    title: "TSP — local delivery loop",
    briefing: `Minimize total distance visiting every city once and returning to city **0**.

**Solver output:** length-\`cities\` permutation (do **not** repeat city 0 at the end — the chain closes the tour).

**Verification:** Hamiltonian cycle; distance sum over the matrix.`,
    instance: {
      cities: 5,
      distances: [
        [0, 10, 15, 20, 25],
        [10, 0, 35, 25, 30],
        [15, 35, 0, 30, 20],
        [20, 25, 30, 0, 15],
        [25, 30, 20, 15, 0],
      ],
    },
    suggestedBounty: "15",
    suggestedMinWorkScore: "200",
    suggestedComplexity: "hard",
    suggestedExpirationDays: "30",
  },
  {
    id: "tsp-us-coldchain",
    kind: "TSP",
    label: "US cold-chain",
    tagline: "15 hubs · road miles · expert",
    title: "US Cold-Chain Vaccine Relay — 15-Hub National TSP",
    briefing: `Route a temperature-controlled relay from **New York (depot, index 0)** through 14 regional hubs exactly once and back. Distances are approximate **US road miles** (symmetric integer matrix).

| Index | City |
|------:|------|
| 0 | New York, NY (depot) |
| 1 | Boston, MA |
| 2 | Philadelphia, PA |
| 3 | Washington, DC |
| 4 | Atlanta, GA |
| 5 | Chicago, IL |
| 6 | Detroit, MI |
| 7 | Dallas, TX |
| 8 | Houston, TX |
| 9 | Denver, CO |
| 10 | Phoenix, AZ |
| 11 | Los Angeles, CA |
| 12 | San Francisco, CA |
| 13 | Seattle, WA |
| 14 | Miami, FL |

**Solver output:** permutation of \`0..14\`. Quality scales vs. nearest-neighbor baseline (~10,464 mi from NYC).`,
    instance: { cities: 15, distances: TSP_US_15_DISTANCES },
    suggestedBounty: "50",
    suggestedMinWorkScore: "200",
    suggestedComplexity: "expert",
    suggestedExpirationDays: "30",
  },
  {
    id: "sat-starter",
    kind: "SAT",
    label: "Starter",
    tagline: "4 variables · 4 clauses",
    title: "SAT — shift scheduling constraints",
    briefing: `Find a boolean assignment satisfying all CNF clauses. Literals use DIMACS sign: positive = variable, negative = negated.

**Solver output:** \`assignment\` array of booleans length \`variables\`.`,
    instance: {
      variables: 4,
      clauses: [
        [1, -2, 3],
        [-1, 2, -3],
        [2, 3, 4],
        [-2, -3, 4],
      ],
    },
    suggestedBounty: "20",
    suggestedMinWorkScore: "180",
    suggestedComplexity: "expert",
    suggestedExpirationDays: "30",
  },
  {
    id: "sat-scheduling",
    kind: "SAT",
    label: "Scheduling",
    tagline: "8 variables · 12 clauses",
    title: "SAT — conference room allocation",
    briefing: `Eight rooms, twelve conflict constraints. Return any satisfying assignment.

Clauses may be literal arrays \`[1,-2,3]\` (supported by the verifier).`,
    instance: {
      variables: 8,
      clauses: [
        [1, 2, -3],
        [-1, 3, 4],
        [2, -4, 5],
        [-2, 5, -6],
        [3, 6, 7],
        [-3, -7, 8],
        [4, -5, -8],
        [1, -6, 8],
        [-1, 7, -8],
        [2, 3, -8],
        [-4, 6, 7],
        [5, -7, 8],
      ],
    },
    suggestedBounty: "25",
    suggestedMinWorkScore: "200",
    suggestedComplexity: "expert",
    suggestedExpirationDays: "30",
  },
];

export function templatesForKind(kind: BountyProblemKind): BountyTemplate[] {
  return BOUNTY_TEMPLATES.filter((t) => t.kind === kind);
}

export function getBountyTemplate(id: string): BountyTemplate | undefined {
  return BOUNTY_TEMPLATES.find((t) => t.id === id);
}

export function instanceToEditorJson(instance: Record<string, unknown>): string {
  return JSON.stringify(instance, null, 2);
}

export function applyTemplateToForm(template: BountyTemplate) {
  return {
    title: template.title,
    problemType: template.kind,
    briefing: template.briefing,
    instanceJson: instanceToEditorJson(template.instance),
    bounty: template.suggestedBounty,
    minWorkScore: template.suggestedMinWorkScore,
    complexity: template.suggestedComplexity,
    expirationDays: template.suggestedExpirationDays,
  };
}
