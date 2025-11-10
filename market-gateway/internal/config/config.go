package config

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/spf13/viper"
)

// Config holds the application configuration
type Config struct {
	Server   ServerConfig   `mapstructure:"server"`
	Logging  LoggingConfig  `mapstructure:"logging"`
	Market   MarketConfig   `mapstructure:"market"`
}

// ServerConfig holds gRPC server connection settings
type ServerConfig struct {
	Address         string `mapstructure:"address"`
	ConnectTimeout  int    `mapstructure:"connect_timeout"`  // seconds
	RequestTimeout  int    `mapstructure:"request_timeout"`  // seconds
	EnableTLS       bool   `mapstructure:"enable_tls"`
	CertFile        string `mapstructure:"cert_file"`
}

// LoggingConfig holds logging settings
type LoggingConfig struct {
	Level      string `mapstructure:"level"`       // debug, info, warn, error
	Format     string `mapstructure:"format"`      // json, console
	OutputPath string `mapstructure:"output_path"` // stdout, stderr, or file path
}

// MarketConfig holds market data settings
type MarketConfig struct {
	DefaultCurrency    string  `mapstructure:"default_currency"`
	DefaultVolatility  float64 `mapstructure:"default_volatility"`
	DefaultRate        float64 `mapstructure:"default_rate"`
	UpdateIntervalMs   int     `mapstructure:"update_interval_ms"`
}

var (
	config *Config
)

// GetConfig returns the current configuration
func GetConfig() *Config {
	if config == nil {
		config = getDefaultConfig()
	}
	return config
}

// getDefaultConfig returns default configuration values
func getDefaultConfig() *Config {
	return &Config{
		Server: ServerConfig{
			Address:        "localhost:50051",
			ConnectTimeout: 5,
			RequestTimeout: 30,
			EnableTLS:      false,
		},
		Logging: LoggingConfig{
			Level:      "info",
			Format:     "console",
			OutputPath: "stdout",
		},
		Market: MarketConfig{
			DefaultCurrency:   "USD",
			DefaultVolatility: 0.12,
			DefaultRate:       0.05,
			UpdateIntervalMs:  1000,
		},
	}
}

// LoadConfig loads configuration from a file
func LoadConfig(configPath string) error {
	viper.SetConfigFile(configPath)

	if err := viper.ReadInConfig(); err != nil {
		return fmt.Errorf("failed to read config file: %w", err)
	}

	if err := viper.Unmarshal(&config); err != nil {
		return fmt.Errorf("failed to unmarshal config: %w", err)
	}

	return nil
}

// LoadDefaultConfig attempts to load config from default locations
func LoadDefaultConfig() {
	// Set default values
	config = getDefaultConfig()

	viper.SetConfigName(".market-gateway")
	viper.SetConfigType("yaml")

	// Add config search paths
	if home, err := os.UserHomeDir(); err == nil {
		viper.AddConfigPath(home)
	}
	viper.AddConfigPath(".")
	viper.AddConfigPath(filepath.Join(".", "config"))

	// Set environment variable prefix
	viper.SetEnvPrefix("MG")
	viper.AutomaticEnv()

	// Try to read config (ignore error if not found)
	if err := viper.ReadInConfig(); err == nil {
		_ = viper.Unmarshal(&config)
	}
}

// Validate validates the configuration
func (c *Config) Validate() error {
	if c.Server.Address == "" {
		return fmt.Errorf("server address cannot be empty")
	}

	if c.Server.ConnectTimeout <= 0 {
		return fmt.Errorf("connect timeout must be positive")
	}

	if c.Server.RequestTimeout <= 0 {
		return fmt.Errorf("request timeout must be positive")
	}

	if c.Server.EnableTLS && c.Server.CertFile == "" {
		return fmt.Errorf("cert file required when TLS is enabled")
	}

	validLevels := map[string]bool{"debug": true, "info": true, "warn": true, "error": true}
	if !validLevels[c.Logging.Level] {
		return fmt.Errorf("invalid log level: %s", c.Logging.Level)
	}

	validFormats := map[string]bool{"json": true, "console": true}
	if !validFormats[c.Logging.Format] {
		return fmt.Errorf("invalid log format: %s", c.Logging.Format)
	}

	if c.Market.DefaultVolatility < 0 || c.Market.DefaultVolatility > 1 {
		return fmt.Errorf("default volatility must be between 0 and 1")
	}

	if c.Market.UpdateIntervalMs < 0 {
		return fmt.Errorf("update interval must be non-negative")
	}

	return nil
}

// ExampleConfig generates an example configuration file content
func ExampleConfig() string {
	return `# Market Gateway Configuration

server:
  address: "localhost:50051"
  connect_timeout: 5  # seconds
  request_timeout: 30 # seconds
  enable_tls: false
  cert_file: ""

logging:
  level: "info"       # debug, info, warn, error
  format: "console"   # json, console
  output_path: "stdout"

market:
  default_currency: "USD"
  default_volatility: 0.12  # 12%
  default_rate: 0.05        # 5%
  update_interval_ms: 1000  # 1 second
`
}
