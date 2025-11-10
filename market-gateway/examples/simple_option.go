package main

import (
	"context"
	"fmt"
	"log"
	"time"

	"go.uber.org/zap"

	"github.com/leonc/ficc-pricer/market-gateway/internal/client"
	"github.com/leonc/ficc-pricer/market-gateway/internal/market"
	"github.com/leonc/ficc-pricer/market-gateway/internal/models"
)

// This example demonstrates how to:
// 1. Set up market data
// 2. Build an FX option contract
// 3. Connect to the pricing service
// 4. Request a price
//
// NOTE: This is a placeholder until protobuf schemas are generated.
// The actual pricing call will work once the proto files are created.

func main() {
	// Initialize logger
	logger, err := zap.NewDevelopment()
	if err != nil {
		log.Fatalf("Failed to create logger: %v", err)
	}
	defer logger.Sync()

	logger.Info("Starting simple option pricing example")

	// Step 1: Create and populate market data
	marketMgr := market.NewManager(logger)

	// Set up EUR/USD spot rate
	if err := marketMgr.UpdateSpotRate("EUR/USD", 1.10); err != nil {
		logger.Fatal("Failed to update spot rate", zap.Error(err))
	}

	// Set up discount curves (flat rates for simplicity)
	if err := marketMgr.UpdateDiscountCurve("USD", 0.05, "continuous"); err != nil {
		logger.Fatal("Failed to update USD curve", zap.Error(err))
	}

	if err := marketMgr.UpdateDiscountCurve("EUR", 0.03, "continuous"); err != nil {
		logger.Fatal("Failed to update EUR curve", zap.Error(err))
	}

	// Set up volatility surface (flat vol)
	if err := marketMgr.UpdateVolSurface("EUR/USD", 0.12); err != nil {
		logger.Fatal("Failed to update vol surface", zap.Error(err))
	}

	// Take a market snapshot
	snapshot := marketMgr.GetSnapshot()
	logger.Info("Market snapshot created",
		zap.Int("spot_rates", len(snapshot.SpotRates)),
		zap.Int("discount_curves", len(snapshot.DiscountCurves)),
		zap.Int("vol_surfaces", len(snapshot.VolSurfaces)),
	)

	// Step 2: Build the contract - EUR/USD Call Option
	maturity := time.Date(2025, 12, 31, 0, 0, 0, 0, time.UTC)
	notional := 1_000_000.0
	strike := 1.15

	contract := models.NewScaledOption(
		notional,
		models.Call,
		strike,
		maturity,
		models.USD,
		models.EUR,
	)

	logger.Info("Contract created", zap.String("contract", contract.String()))

	// Step 3: Connect to pricing service
	pricerClient, err := client.NewPricerClient("localhost:50051", logger)
	if err != nil {
		logger.Fatal("Failed to create pricer client", zap.Error(err))
	}
	defer pricerClient.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	logger.Info("Attempting to connect to pricing service...")

	// NOTE: This will fail until the Haskell gRPC service is running
	err = pricerClient.Connect(ctx)
	if err != nil {
		logger.Warn("Could not connect to pricing service (expected if service not running)",
			zap.Error(err),
		)
		logger.Info("To run the pricing service, implement the Haskell gRPC server")
		return
	}

	logger.Info("Successfully connected to pricing service")

	// Step 4: Request price (placeholder until protobuf is generated)
	err = pricerClient.PriceRequest(ctx)
	if err != nil {
		logger.Error("Price request failed", zap.Error(err))
		return
	}

	// Once protobuf is generated, this will look like:
	/*
		priceReq := &pb.PriceRequest{
			Contract: contractToProto(contract),
			Market:   snapshotToProto(snapshot),
			Params: &pb.PricingParams{
				ValuationDate: time.Now().Format(time.RFC3339),
				Numeraire:     pb.Currency_USD,
				Model:         pb.PricingModel_BLACK_SCHOLES,
			},
		}

		resp, err := pricerClient.Price(ctx, priceReq)
		if err != nil {
			logger.Error("Price request failed", zap.Error(err))
			return
		}

		logger.Info("Price received",
			zap.Float64("price", resp.Price),
			zap.String("numeraire", resp.Numeraire),
			zap.Float64("computation_time_ms", resp.ComputationTimeMs),
		)
	*/

	fmt.Println("\n=== Example Summary ===")
	fmt.Printf("Contract: %s\n", contract.String())
	fmt.Printf("Spot: EUR/USD = %.4f\n", snapshot.SpotRates["EUR/USD"].Rate)
	fmt.Printf("USD Rate: %.2f%%\n", snapshot.DiscountCurves["USD"].FlatRate*100)
	fmt.Printf("EUR Rate: %.2f%%\n", snapshot.DiscountCurves["EUR"].FlatRate*100)
	fmt.Printf("Volatility: %.2f%%\n", snapshot.VolSurfaces["EUR/USD"].FlatVol*100)
	fmt.Println("\nNext steps:")
	fmt.Println("1. Define protobuf schema (pricer.proto)")
	fmt.Println("2. Generate Go code: protoc --go_out=. --go-grpc_out=. pricer.proto")
	fmt.Println("3. Implement Haskell gRPC server")
	fmt.Println("4. Update this example with real pricing calls")
}
