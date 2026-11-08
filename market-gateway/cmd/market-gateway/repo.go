package main

import (
	"fmt"
	"io"
	"os"
	"time"

	"github.com/spf13/cobra"

	"github.com/leonc/ficc-pricer/market-gateway/internal/market"
)

// repoCmd groups the repo market-data commands that feed the Rust
// repo-analytics crate (see repo-analytics/src/market.rs).
var repoCmd = &cobra.Command{
	Use:   "repo",
	Short: "Repo market data for the Rust repo pricer",
	Long: `Build, export and validate repo market snapshots (GC curves, specials,
overnight fixings, collateral static, clean prices and haircuts) in the JSON
format read by repo-analytics.`,
}

var repoExportCmd = &cobra.Command{
	Use:          "export",
	SilenceUsage: true,
	Short:        "Export a repo market snapshot as JSON",
	Long: `Export the simulated repo market as of a business date.

Examples:
  market-gateway repo export --asof 2026-11-09 --out snapshot.json
  market-gateway repo export | cargo run --features serde --example repo_report_from_snapshot -- -`,
	RunE: func(cmd *cobra.Command, args []string) error {
		asofStr, _ := cmd.Flags().GetString("asof")
		out, _ := cmd.Flags().GetString("out")

		asof := market.NewDate(time.Now())
		if asofStr != "" {
			var err error
			if asof, err = market.ParseDate(asofStr); err != nil {
				return err
			}
		}
		mgr := market.NewManager(logger)
		if err := market.LoadSimulatedRepoMarket(mgr, asof); err != nil {
			return fmt.Errorf("build simulated repo market: %w", err)
		}
		snap, err := mgr.GetRepoSnapshot()
		if err != nil {
			return err
		}
		snap.GeneratedAt = time.Now().UTC().Format(time.RFC3339)
		data, err := market.MarshalRepoSnapshot(snap)
		if err != nil {
			return err
		}
		if out == "-" {
			_, err = os.Stdout.Write(data)
			return err
		}
		if err := os.WriteFile(out, data, 0o644); err != nil {
			return err
		}
		fmt.Fprintf(os.Stderr, "wrote repo snapshot as of %s to %s\n", asof, out)
		return nil
	},
}

var repoValidateCmd = &cobra.Command{
	Use:          "validate [file|-]",
	SilenceUsage: true,
	Short:        "Validate a repo market snapshot",
	Args:         cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		var data []byte
		var err error
		if args[0] == "-" {
			data, err = io.ReadAll(os.Stdin)
		} else {
			data, err = os.ReadFile(args[0])
		}
		if err != nil {
			return err
		}
		snap, err := market.UnmarshalRepoSnapshot(data)
		if err != nil {
			return err
		}
		mgr := market.NewManager(logger)
		if err := mgr.LoadRepoSnapshot(snap); err != nil {
			return err
		}
		fmt.Printf("valid repo snapshot (schema v%d) as of %s: %v\n", snap.SchemaVersion, snap.AsOf, mgr.RepoStats())
		return nil
	},
}

func init() {
	rootCmd.AddCommand(repoCmd)
	repoCmd.AddCommand(repoExportCmd, repoValidateCmd)
	repoExportCmd.Flags().String("asof", "", "business date YYYY-MM-DD (default: today)")
	repoExportCmd.Flags().StringP("out", "o", "-", "output file, or - for stdout")
}
