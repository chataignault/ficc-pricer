package models

import (
	"fmt"
	"time"
)

// Currency represents a currency code
type Currency int

const (
	USD Currency = iota
	EUR
	GBP
	JPY
	CHF
	AUD
	CAD
)

var currencyNames = map[Currency]string{
	USD: "USD",
	EUR: "EUR",
	GBP: "GBP",
	JPY: "JPY",
	CHF: "CHF",
	AUD: "AUD",
	CAD: "CAD",
}

func (c Currency) String() string {
	return currencyNames[c]
}

// ParseCurrency parses a currency string to Currency type
func ParseCurrency(s string) (Currency, error) {
	for c, name := range currencyNames {
		if name == s {
			return c, nil
		}
	}
	return 0, fmt.Errorf("unknown currency: %s", s)
}

// OptionType represents call or put option
type OptionType int

const (
	Call OptionType = iota
	Put
)

func (o OptionType) String() string {
	if o == Call {
		return "CALL"
	}
	return "PUT"
}

// Contract is a marker interface for all contract types
// This will map to the Haskell Contract GADT via protobuf
type Contract interface {
	isContract()
	String() string
}

// Zero represents a contract with zero value
type Zero struct{}

func (Zero) isContract()     {}
func (Zero) String() string { return "Zero" }

// Spot represents a spot FX contract
type Spot struct {
	Domestic Currency
	Foreign  Currency
}

func (s Spot) isContract() {}
func (s Spot) String() string {
	return fmt.Sprintf("Spot(%s/%s)", s.Foreign, s.Domestic)
}

// Forward represents an FX forward contract
type Forward struct {
	Maturity  time.Time
	FixedRate float64
	Domestic  Currency
	Foreign   Currency
}

func (f Forward) isContract() {}
func (f Forward) String() string {
	return fmt.Sprintf("Forward(%s/%s, Strike: %.4f, Maturity: %s)",
		f.Foreign, f.Domestic, f.FixedRate, f.Maturity.Format("2006-01-02"))
}

// EurOption represents a European option
type EurOption struct {
	Type     OptionType
	Strike   float64
	Maturity time.Time
	Domestic Currency
	Foreign  Currency
}

func (e EurOption) isContract() {}
func (e EurOption) String() string {
	return fmt.Sprintf("EurOption(%s, %s/%s, Strike: %.4f, Maturity: %s)",
		e.Type, e.Foreign, e.Domestic, e.Strike, e.Maturity.Format("2006-01-02"))
}

// ZCB represents a zero-coupon bond
type ZCB struct {
	Currency Currency
	Maturity time.Time
}

func (z ZCB) isContract() {}
func (z ZCB) String() string {
	return fmt.Sprintf("ZCB(%s, Maturity: %s)",
		z.Currency, z.Maturity.Format("2006-01-02"))
}

// Scale represents a scaled contract (contract multiplied by notional)
type Scale struct {
	Notional float64
	Contract Contract
}

func (s Scale) isContract() {}
func (s Scale) String() string {
	return fmt.Sprintf("Scale(%.2f, %s)", s.Notional, s.Contract)
}

// Combine represents a combination of two contracts
type Combine struct {
	Left  Contract
	Right Contract
}

func (c Combine) isContract() {}
func (c Combine) String() string {
	return fmt.Sprintf("Combine(%s, %s)", c.Left, c.Right)
}

// Builder functions for ergonomic contract construction

// NewSpot creates a new spot contract
func NewSpot(domestic, foreign Currency) Spot {
	return Spot{
		Domestic: domestic,
		Foreign:  foreign,
	}
}

// NewForward creates a new forward contract
func NewForward(maturity time.Time, fixedRate float64, domestic, foreign Currency) Forward {
	return Forward{
		Maturity:  maturity,
		FixedRate: fixedRate,
		Domestic:  domestic,
		Foreign:   foreign,
	}
}

// NewEurOption creates a new European option
func NewEurOption(optType OptionType, strike float64, maturity time.Time, domestic, foreign Currency) EurOption {
	return EurOption{
		Type:     optType,
		Strike:   strike,
		Maturity: maturity,
		Domestic: domestic,
		Foreign:  foreign,
	}
}

// NewZCB creates a new zero-coupon bond
func NewZCB(currency Currency, maturity time.Time) ZCB {
	return ZCB{
		Currency: currency,
		Maturity: maturity,
	}
}

// NewScale creates a scaled contract
func NewScale(notional float64, contract Contract) Scale {
	return Scale{
		Notional: notional,
		Contract: contract,
	}
}

// NewCombine creates a combined contract
func NewCombine(left, right Contract) Combine {
	return Combine{
		Left:  left,
		Right: right,
	}
}

// Example helper functions for common patterns

// NewCallOption creates a call option
func NewCallOption(strike float64, maturity time.Time, domestic, foreign Currency) EurOption {
	return NewEurOption(Call, strike, maturity, domestic, foreign)
}

// NewPutOption creates a put option
func NewPutOption(strike float64, maturity time.Time, domestic, foreign Currency) EurOption {
	return NewEurOption(Put, strike, maturity, domestic, foreign)
}

// NewScaledOption creates a scaled European option (common use case)
func NewScaledOption(notional float64, optType OptionType, strike float64, maturity time.Time, domestic, foreign Currency) Scale {
	option := NewEurOption(optType, strike, maturity, domestic, foreign)
	return NewScale(notional, option)
}
