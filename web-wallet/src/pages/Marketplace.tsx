import { useEffect, useState } from 'react';
import { MarketplaceClient, CatalogStats, MarketplaceCatalogEntry } from '../lib/marketplace-client';
import BountySubmissionForm from '../components/BountySubmissionForm';
import RevealProblemForm from '../components/RevealProblemForm';

const client = new MarketplaceClient();

export default function Marketplace() {
  const [stats, setStats] = useState<CatalogStats | null>(null);
  const [recentEntries, setRecentEntries] = useState<MarketplaceCatalogEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showSubmitForm, setShowSubmitForm] = useState(false);
  const [showRevealForm, setShowRevealForm] = useState(false);

  useEffect(() => {
    loadData();
    const interval = setInterval(loadData, 5000); // Refresh every 5s
    return () => clearInterval(interval);
  }, []);

  const loadData = async () => {
    try {
      const [statsData, searchResults] = await Promise.all([
        client.getStats(),
        client.search({ limit: 10, offset: 0 })
      ]);
      setStats(statsData);
      setRecentEntries(searchResults.entries);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load marketplace data');
    } finally {
      setLoading(false);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="text-xl">Loading marketplace...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="text-red-500">
          <p className="text-xl font-bold">Error</p>
          <p>{error}</p>
          <p className="text-sm mt-2">Make sure the marketplace-export service is running on port 8080</p>
        </div>
      </div>
    );
  }

  if (!stats) return null;

  const problemTypes = [
    { name: 'SubsetSum', count: stats.by_type.subset_sum, color: 'bg-blue-500' },
    { name: 'SAT', count: stats.by_type.sat, color: 'bg-green-500' },
    { name: 'TSP', count: stats.by_type.tsp, color: 'bg-yellow-500' },
    { name: 'Custom', count: stats.by_type.custom, color: 'bg-purple-500' },
  ];

  const maxCount = Math.max(...problemTypes.map(p => p.count));

  return (
    <div className="container mx-auto p-6 space-y-6">
      <div className="flex justify-between items-center">
        <h1 className="text-3xl font-bold">PoUW Marketplace Catalog</h1>
        <div className="flex gap-3">
          <button
            onClick={() => setShowSubmitForm(true)}
            className="bg-blue-600 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded transition-colors"
          >
            Submit Bounty
          </button>
          <button
            onClick={() => setShowRevealForm(true)}
            className="bg-green-600 hover:bg-green-700 text-white font-bold py-2 px-4 rounded transition-colors"
          >
            Reveal Problem
          </button>
        </div>
      </div>

      {/* Stats Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <StatCard
          title="Total Problems"
          value={stats.total_problems.toLocaleString()}
          subtitle={`Latest block: ${stats.latest_height}`}
        />
        <StatCard
          title="Avg Work Score"
          value={stats.work_score_stats.avg.toFixed(2)}
          subtitle={`Range: ${stats.work_score_stats.min.toFixed(1)}-${stats.work_score_stats.max.toFixed(1)}`}
        />
        <StatCard
          title="Avg Time Asymmetry"
          value={`${stats.time_asymmetry_stats.avg.toFixed(1)}x`}
          subtitle={`Max: ${stats.time_asymmetry_stats.max.toFixed(1)}x`}
        />
        <StatCard
          title="Total Energy"
          value={`${(stats.total_energy_joules / 1000).toFixed(1)} kJ`}
          subtitle={`${(stats.total_energy_joules / 3600000).toFixed(2)} kWh`}
        />
      </div>

      {/* Problem Type Distribution */}
      <div className="bg-gray-800 rounded-lg p-6">
        <h2 className="text-xl font-bold mb-4">Problem Type Distribution</h2>
        <div className="space-y-3">
          {problemTypes.map((type) => (
            <div key={type.name} className="flex items-center gap-4">
              <div className="w-24 text-sm font-medium">{type.name}</div>
              <div className="flex-1">
                <div className="bg-gray-700 rounded-full h-6 overflow-hidden">
                  <div
                    className={`${type.color} h-full flex items-center justify-end pr-2 text-xs font-bold transition-all`}
                    style={{ width: `${maxCount > 0 ? (type.count / maxCount) * 100 : 0}%` }}
                  >
                    {type.count > 0 && type.count}
                  </div>
                </div>
              </div>
              <div className="w-16 text-right text-sm text-gray-400">
                {stats.total_problems > 0
                  ? ((type.count / stats.total_problems) * 100).toFixed(1)
                  : 0}%
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Recent Entries */}
      <div className="bg-gray-800 rounded-lg p-6">
        <h2 className="text-xl font-bold mb-4">Recent Problem Solutions</h2>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead className="border-b border-gray-700">
              <tr className="text-left text-gray-400">
                <th className="pb-2">Block</th>
                <th className="pb-2">Problem ID</th>
                <th className="pb-2">Work Score</th>
                <th className="pb-2">Quality</th>
                <th className="pb-2">Time Asymmetry</th>
                <th className="pb-2">Energy (J)</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-700">
              {recentEntries.map((entry) => (
                <tr key={entry.problem_id} className="hover:bg-gray-750">
                  <td className="py-2">{entry.provenance.block_height}</td>
                  <td className="py-2 font-mono text-xs">{entry.problem_id.substring(0, 12)}...</td>
                  <td className="py-2">{entry.metrics.work_score.toFixed(2)}</td>
                  <td className="py-2">
                    <QualityBadge quality={entry.metrics.solution_quality} />
                  </td>
                  <td className="py-2">{entry.metrics.time_asymmetry_ratio.toFixed(1)}x</td>
                  <td className="py-2">{entry.metrics.energy_estimate_joules.toFixed(1)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      {/* Bounty Submission Modal */}
      {showSubmitForm && (
        <BountySubmissionForm
          onClose={() => setShowSubmitForm(false)}
          onSuccess={() => loadData()}
        />
      )}

      {/* Reveal Problem Modal */}
      {showRevealForm && (
        <RevealProblemForm
          onClose={() => setShowRevealForm(false)}
          onSuccess={() => loadData()}
        />
      )}
    </div>
  );
}

function StatCard({ title, value, subtitle }: { title: string; value: string; subtitle: string }) {
  return (
    <div className="bg-gray-800 rounded-lg p-4">
      <div className="text-sm text-gray-400 mb-1">{title}</div>
      <div className="text-2xl font-bold mb-1">{value}</div>
      <div className="text-xs text-gray-500">{subtitle}</div>
    </div>
  );
}

function QualityBadge({ quality }: { quality: number }) {
  const percentage = (quality * 100).toFixed(1);
  const colorClass = quality >= 0.9 ? 'bg-green-500/20 text-green-400' : quality >= 0.7 ? 'bg-yellow-500/20 text-yellow-400' : 'bg-red-500/20 text-red-400';

  return (
    <span className={`px-2 py-1 rounded text-xs font-medium ${colorClass}`}>
      {percentage}%
    </span>
  );
}
