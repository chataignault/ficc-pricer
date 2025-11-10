package main

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
	"go.uber.org/zap"

	"github.com/leonc/ficc-pricer/market-gateway/internal/config"
)

var (
	cfgFile string
	logger  *zap.Logger
)

// rootCmd represents the base command when called without any subcommands
var rootCmd = &cobra.Command{
	Use:   "market-gateway",
	Short: "Market Gateway - gRPC client for FX Pricing Service",
	Long: `Market Gateway is a Go application that orchestrates pricing requests
and manages market data updates for the Haskell FX pricing service.

It handles:
- Communication with the Haskell pricing service via gRPC
- Market data state management (spots, curves, volatilities)
- Price request orchestration
- Market data snapshots and updates`,
	PersistentPreRun: func(cmd *cobra.Command, args []string) {
		// Initialize logger
		var err error
		logger, err = zap.NewDevelopment()
		if err != nil {
			fmt.Fprintf(os.Stderr, "failed to initialize logger: %v\n", err)
			os.Exit(1)
		}
	},
	PersistentPostRun: func(cmd *cobra.Command, args []string) {
		// Cleanup logger
		if logger != nil {
			_ = logger.Sync()
		}
	},
}

// priceCmd represents the price command
var priceCmd = &cobra.Command{
	Use:   "price",
	Short: "Request a price for an FX contract",
	Long: `Send a pricing request to the Haskell pricing service.

Example:
  market-gateway price --contract option --strike 1.15 --maturity 2025-12-31`,
	Run: func(cmd *cobra.Command, args []string) {
		logger.Info("price command called (not yet implemented)")
		fmt.Println("Price command - to be implemented with protobuf generation")
	},
}

// updateCmd represents the market update command
var updateCmd = &cobra.Command{
	Use:   "update",
	Short: "Push market data updates to the pricing service",
	Long: `Update market data in the Haskell pricing service.
Supports spot rates, discount curves, and volatility surfaces.

Example:
  market-gateway update --spot EUR/USD=1.1050`,
	Run: func(cmd *cobra.Command, args []string) {
		logger.Info("update command called (not yet implemented)")
		fmt.Println("Update command - to be implemented with protobuf generation")
	},
}

// serveCmd represents the serve command (for future daemon mode)
var serveCmd = &cobra.Command{
	Use:   "serve",
	Short: "Run market gateway in daemon mode",
	Long: `Start the market gateway as a long-running daemon that continuously
manages market data and handles pricing requests.`,
	Run: func(cmd *cobra.Command, args []string) {
		logger.Info("serve command called (not yet implemented)")
		fmt.Println("Serve command - to be implemented")
	},
}

// healthCmd represents the health check command
var healthCmd = &cobra.Command{
	Use:   "health",
	Short: "Check pricing service health",
	Long:  `Query the health status of the Haskell pricing service.`,
	Run: func(cmd *cobra.Command, args []string) {
		logger.Info("health command called (not yet implemented)")
		fmt.Println("Health command - to be implemented with protobuf generation")
	},
}

func init() {
	cobra.OnInitialize(initConfig)

	// Persistent flags for all commands
	rootCmd.PersistentFlags().StringVar(&cfgFile, "config", "", "config file (default is $HOME/.market-gateway.yaml)")
	rootCmd.PersistentFlags().String("server", "localhost:50051", "address of the pricing service")

	// Add subcommands
	rootCmd.AddCommand(priceCmd)
	rootCmd.AddCommand(updateCmd)
	rootCmd.AddCommand(serveCmd)
	rootCmd.AddCommand(healthCmd)

	// Price command flags (placeholders)
	priceCmd.Flags().String("contract", "option", "contract type (spot, forward, option)")
	priceCmd.Flags().Float64("strike", 0, "option strike price")
	priceCmd.Flags().String("maturity", "", "contract maturity date (ISO 8601)")

	// Update command flags (placeholders)
	updateCmd.Flags().String("spot", "", "spot rate update (format: CCY1/CCY2=rate)")
}

func initConfig() {
	if cfgFile != "" {
		// Use config file from the flag
		config.LoadConfig(cfgFile)
	} else {
		// Use default config locations
		config.LoadDefaultConfig()
	}
}

func main() {
	if err := rootCmd.Execute(); err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
}
