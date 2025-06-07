# Aero-Arb-MM Bot v0.5.0

🛩️ A sophisticated DeFi bot for arbitrage detection, market-making simulation, and trade execution simulation on Aerodrome DEX (Base L2).

## 🌟 Features

### 🎯 Arbitrage Detection
- **Real-time Monitoring**: Continuously monitors WETH/USD pools on Aerodrome vs Binance prices
- **Enhanced Validation**: Multi-layer validation including price sanity, liquidity checks, gas economics, and volatility assessment
- **Profit Analysis**: Calculates gross profit, gas costs, net profit, and ROI for each opportunity
- **Risk Management**: Ensures trade sizes don't exceed pool impact thresholds

### 🚀 Trade Execution Simulation
- **Testnet-Safe Execution**: Simulates real trades on Base Sepolia testnet for safety
- **Realistic Modeling**: Includes gas costs, slippage, network latency, and failure scenarios
- **Execution Analytics**: Tracks simulation success rates, actual vs expected profits
- **Risk-Free Testing**: No real funds at risk during development and testing

### 🎯 Advanced Market-Making Simulation
- **5 Intelligent Strategies**: TightSpread, WideSpread, InventoryManagement, TrendFollowing, VolatilityAdaptive
- **Multi-Timeframe Volatility**: Analyzes 5-minute, 30-minute, and 1-hour volatility windows
- **Dynamic Spread Calculation**: Automatically adjusts spreads based on volatility, inventory imbalance, and market conditions
- **Fair Value Pricing**: Uses Binance ETH/USDC as fair value reference for optimal bid/ask placement
- **Enhanced Risk Assessment**: Comprehensive risk scoring including VaR, inventory risk, liquidity risk, and volatility risk

### 📈 Enhanced Volatility Analysis
- **Multi-Timeframe Tracking**: Short-term (5m), medium-term (30m), and long-term (1h) volatility
- **Volatility Impact Assessment**: Categorizes market conditions as Low, Moderate, High, or Extreme
- **Trend Detection**: Identifies increasing, decreasing, stable, or volatile market trends
- **Dynamic Adjustments**: Automatically adjusts spreads, position sizes, and execution urgency based on volatility
- **Risk-Aware Execution**: Higher volatility triggers more conservative position sizing and wider spreads

### 🛡️ Enterprise-Grade Reliability
- **Circuit Breaker**: Automatic system protection with configurable error thresholds
- **Retry Logic**: Exponential backoff with jitter for network resilience
- **Error Recovery**: Sophisticated error classification and recovery strategies
- **Health Monitoring**: Real-time system health checks and diagnostics

### 🐳 Containerization
- **Docker Support**: Production-ready Dockerfile with multi-stage builds
- **Docker Compose**: Easy deployment with environment configuration
- **Resource Optimization**: Minimal image size with security best practices
- **Monitoring Ready**: Optional Prometheus and Grafana integration

## 🏗️ Design Choices & Rationale

### AMM Pool Selection
We chose to monitor **Aerodrome Finance** pools on Base for several key reasons:

1. **ve(3,3) Model Innovation**: Aerodrome implements the ve(3,3) tokenomics model, which provides more stable liquidity and better incentive alignment than traditional AMMs
2. **Base Network Focus**: As Base's leading DEX by TVL, Aerodrome offers the deepest liquidity for ETH/USD pairs
3. **Multiple Pool Types**: Aerodrome supports both volatile (x*y=k) and stable pools, allowing for diverse market-making strategies
4. **Active Development**: Regular updates and improvements make it an ideal platform for advanced trading strategies

Specifically, we monitor:
- **vAMM-WETH/USDbC**: The primary volatile ETH/USD pool with highest volume
- **WETH/USDC**: Secondary pool for arbitrage diversity

### Arbitrage Detection Logic
Our arbitrage detection implements a sophisticated multi-layer approach:

1. **Price Discovery**: 
   - Fetch real-time prices from both DEX (on-chain) and CEX (Binance API)
   - Calculate effective spot prices considering pool reserves and fees

2. **Opportunity Identification**:
   - Minimum price difference threshold: 0.05% (configurable)
   - Direction detection: Buy on DEX/Sell on CEX or vice versa
   - Size optimization based on pool liquidity depth

3. **Profit Calculation**:
   ```
   Gross Profit = Trade Size × |DEX Price - CEX Price|
   Gas Cost = Base Fee + Priority Fee (estimated at 150,000 gas units)
   Net Profit = Gross Profit - Gas Cost - Slippage
   ROI = Net Profit / (Trade Size × CEX Price) × 100
   ```

4. **Validation Layers**:
   - Price sanity checks (max 10% deviation)
   - Liquidity depth validation
   - Gas economics verification
   - Slippage estimation with volatility adjustment
   - Pool impact assessment (<1% of reserves)

### Market-Making Strategy Principles

Our simulated market-making strategies follow these core principles:

1. **Fair Value Reference**: 
   - Use CEX (Binance) price as the fair value anchor
   - Adjust for on-chain/off-chain price lag

2. **Dynamic Spread Calculation**:
   - Base spread: 30 bps (0.3%)
   - Volatility multiplier: 1.0-3.0x based on market conditions
   - Inventory adjustment: ±10-50% based on position imbalance
   - Liquidity depth factor: Wider spreads in thin markets

3. **Range Determination**:
   - **Tight markets** (<2% volatility): ±0.5% from fair value
   - **Normal markets** (2-5% volatility): ±1% from fair value  
   - **Volatile markets** (5-10% volatility): ±2% from fair value
   - **Extreme conditions** (>10% volatility): ±3% or hold positions

4. **Strategy Selection Logic**:
   - **TightSpread**: Stable conditions, high liquidity
   - **WideSpread**: Capture volatility premium
   - **InventoryManagement**: Rebalance skewed positions
   - **TrendFollowing**: Align with market direction
   - **VolatilityAdaptive**: Dynamic adjustment to rapid changes

## 💬 Discussion Points

### 1. Extending to Execute Real Arbitrage Trades on Base

**Key Challenges:**

1. **MEV Competition**: 
   - Base uses a first-come-first-served mempool, creating intense competition
   - Solution: Deploy dedicated RPC nodes, use flashbots-style private mempools when available

2. **Slippage & Front-running**:
   - Large trades face sandwich attacks and adverse price movement
   - Solution: Split orders, use commit-reveal schemes, implement slippage protection

3. **Gas Price Volatility**:
   - Spikes during high activity can eliminate profits
   - Solution: Dynamic gas pricing, profit threshold buffers, gas price oracles

4. **Latency & Reliability**:
   - Network delays can cause missed opportunities
   - Solution: Colocated infrastructure, redundant RPC endpoints, optimistic execution

5. **Capital Efficiency**:
   - Need sufficient capital on both DEX and CEX
   - Solution: Flash loans, capital recycling strategies, cross-margin systems

**Mitigation Strategies:**
- Implement MEV protection using private mempools
- Use multiple RPC providers with failover
- Deploy smart contracts for atomic arbitrage execution
- Implement dynamic position sizing based on available liquidity
- Monitor mempool for competing transactions

### 2. Market-Making on Aerodrome vs Traditional AMMs

**Aerodrome (ve(3,3)) Specific Complexities:**

1. **Voting Dynamics**:
   - veAERO holders direct emissions to pools
   - Strategy must consider emission schedules and voting patterns
   - Opportunity: Align with large veAERO holders for better rewards

2. **Bribes & Incentives**:
   - Pools compete for votes through bribes
   - Total rewards = Trading fees + Emissions + Bribes
   - Complexity: Optimize for total yield, not just trading profit

3. **Stable vs Volatile Pools**:
   - Different fee structures (0.01% stable, 0.3% volatile)
   - Stable pools use specialized curve for correlated assets
   - Strategy must adapt to pool type

**Uniswap V4 on Base Complexities:**

1. **Hooks Architecture**:
   - Custom logic can modify pool behavior
   - Strategies must account for hook-specific mechanics
   - Opportunity: Create custom hooks for advanced strategies

2. **Dynamic Fees**:
   - Fees can adjust based on market conditions
   - More complex profitability calculations
   - Need to model fee dynamics in strategy

3. **Singleton Contract**:
   - All pools in one contract reduces gas costs
   - But increases systemic risk considerations
   - Flash accounting enables new strategy types

**Additional Complexities vs Simple AMMs:**
- Position NFTs require different management approach
- Concentrated liquidity demands active range management
- Impermanent loss is path-dependent and more complex
- Gas costs for position adjustments must be modeled
- Multi-pool strategies for optimal capital efficiency

### 3. Private Key Management & Security

**Production Security Architecture:**

1. **Key Storage**:
   - **Hardware Security Modules (HSMs)**: Store keys in FIPS 140-2 Level 3 certified devices
   - **Multi-signature Wallets**: Require M-of-N signatures for transactions
   - **Key Sharding**: Split keys using Shamir's Secret Sharing

2. **Access Control**:
   - **Role-Based Access**: Separate keys for different operations
   - **Time-locks**: Delay sensitive operations
   - **Spending Limits**: Daily/hourly transaction limits

3. **Infrastructure Security**:
   ```
   ┌─────────────────┐     ┌──────────────┐     ┌─────────────┐
   │   Secure VPC    │────▶│ Bastion Host │────▶│   Bot Pod   │
   │   (No Internet) │     │  (MFA Auth)  │     │  (Read-only)│
   └─────────────────┘     └──────────────┘     └─────────────┘
                                  │
                                  ▼
                          ┌──────────────┐
                          │   HSM/KMS    │
                          │ (Sign Only)  │
                          └──────────────┘
   ```

4. **Operational Security**:
   - **Audit Logging**: Every key usage logged immutably
   - **Anomaly Detection**: Alert on unusual signing patterns
   - **Emergency Procedures**: Clear key rotation/revocation process
   - **Insurance**: Consider on-chain insurance protocols

5. **Best Practices**:
   - Never store keys in environment variables or code
   - Use dedicated signing services (AWS KMS, HashiCorp Vault)
   - Implement transaction simulation before signing
   - Regular security audits and penetration testing
   - Separate hot/warm/cold wallet architecture

## 🚀 Infrastructure & Production Deployment

### 1. Key Components of Production Infrastructure

**Core Architecture for L2 (Base) Operations:**

```
┌──────────────────────────────────────────────────────────┐
│                   Load Balancer (HA)                      │
└─────────────┬────────────────────────┬───────────────────┘
              │                        │
     ┌────────▼────────┐      ┌────────▼────────┐
     │   Bot Instance  │      │   Bot Instance  │
     │   (Primary)     │      │   (Secondary)   │
     └────────┬────────┘      └────────┬────────┘
              │                        │
     ┌────────▼────────────────────────▼────────┐
     │        Message Queue (Kafka/Redis)        │
     └────────┬────────────────────────┬────────┘
              │                        │
     ┌────────▼────────┐      ┌────────▼────────┐
     │   Base RPC      │      │  Backup RPC     │
     │   (Alchemy)     │      │  (Infura)       │
     └─────────────────┘      └─────────────────┘
```

**Essential Components:**

1. **RPC Infrastructure**:
   - Primary: Dedicated Alchemy/Infura endpoint with higher rate limits
   - Backup: Multiple fallback providers (QuickNode, Ankr)
   - Local: Archive node for historical data and reduced latency

2. **Data Pipeline**:
   - **TimescaleDB**: Store time-series price data
   - **Redis**: Cache recent prices and pool states
   - **Kafka**: Event streaming for real-time processing

3. **Execution Engine**:
   - **Kubernetes**: Container orchestration for scaling
   - **Istio**: Service mesh for internal communication
   - **ArgoCD**: GitOps deployment management

4. **Security Layer**:
   - **Vault**: Secret management
   - **WAF**: Protect API endpoints
   - **VPN**: Secure internal communication

### 2. Monitoring & Alerting Systems

**Key Metrics to Track:**

**Business Metrics:**
- Arbitrage opportunities detected/executed per hour
- Success rate of executions
- Total profit/loss (realized and unrealized)
- Gas costs vs revenue ratio
- Slippage vs expected

**L2-Specific Metrics:**
- Base network gas prices (L2 fees)
- L1 data availability costs
- Sequencer uptime and latency
- Reorg frequency and depth
- Time to finality on L1

**System Metrics:**
- RPC request latency (p50, p95, p99)
- WebSocket connection stability
- Memory/CPU usage per strategy
- Queue depths and processing lag

**Monitoring Stack:**
```yaml
monitoring:
  - Prometheus: Metrics collection
  - Grafana: Visualization dashboards
  - AlertManager: Alert routing
  - PagerDuty: On-call management
  - Datadog: APM and logging
```

**Critical Alerts:**
- Price divergence > 5% between sources
- RPC errors > 10/minute
- Execution failures > 20%
- Circuit breaker activation
- Sequencer downtime
- Gas price spike > 3x average

### 3. CI/CD Pipeline - Suggestion

**Pipeline Architecture:**

```yaml
name: Production Deployment Pipeline

stages:
  - build
  - test
  - security
  - deploy
  - monitor

build:
  stage: build
  script:
    - cargo build --release
    - docker build -t aero-bot:$CI_COMMIT_SHA .
    - docker push registry/aero-bot:$CI_COMMIT_SHA

test:
  stage: test
  parallel:
    - unit_tests:
        script: cargo test --all
    - integration_tests:
        script: ./scripts/integration_test.sh
    - load_tests:
        script: k6 run load_test.js

security:
  stage: security
  parallel:
    - dependency_scan:
        script: cargo audit
    - container_scan:
        script: trivy image registry/aero-bot:$CI_COMMIT_SHA
    - static_analysis:
        script: cargo clippy -- -D warnings

deploy_staging:
  stage: deploy
  script:
    - kubectl set image deployment/aero-bot aero-bot=registry/aero-bot:$CI_COMMIT_SHA -n staging
    - ./scripts/smoke_test.sh staging
  only:
    - main

deploy_production:
  stage: deploy
  script:
    - kubectl set image deployment/aero-bot aero-bot=registry/aero-bot:$CI_COMMIT_SHA -n production
    - ./scripts/canary_deploy.sh
  when: manual
  only:
    - main

monitor:
  stage: monitor
  script:
    - ./scripts/post_deploy_verification.sh
    - ./scripts/alert_on_anomalies.sh
```

**Key CI/CD Practices:**
1. **Automated Testing**: Unit, integration, and performance tests
2. **Security Scanning**: Dependencies, containers, and code
3. **Canary Deployments**: Gradual rollout with automated rollback
4. **Feature Flags**: Enable/disable strategies without deployment
5. **Rollback Strategy**: Instant reversion to previous version
6. **Audit Trail**: Complete deployment history and approvals

## 🚀 Quick Start

### Prerequisites
- Rust 1.70+ installed OR Docker
- Base network RPC access (Alchemy account)
- Binance API access (no keys required for price data)
- Optional: Base Sepolia testnet ETH for trade execution testing

### Installation

#### Option 1: Docker (Recommended)
```bash
# Clone the repository
git clone <your-repo-url>
cd aero-arb-mm-bot

# Create environment file
cp .env.example .env
# Edit .env with your ALCHEMY_API_KEY

# Run with Docker Compose
docker-compose up -d

# View logs
docker-compose logs -f aero-arb-mm-bot
```

#### Option 2: Native Rust
```bash
# Clone and build
git clone <your-repo-url>
cd aero-arb-mm-bot
cargo build --release

# Create output directories
mkdir -p output/{logs,opportunities,market_making,executions,reports}

# Run the bot
ALCHEMY_API_KEY=your_key cargo run --release
```

### Configuration

Create a `.env` file or set environment variables:

```bash
# Required
ALCHEMY_API_KEY=your_alchemy_api_key

# Network configuration
NETWORK=mainnet                    # or "sepolia" for testnet
RUST_LOG=info                      # or "debug" for verbose logs
EXECUTION_NETWROK=sepolia

# Arbitrage settings
TRADE_SIZE_ETH=0.1                 # Trade size in ETH
MIN_PROFIT_USD=0.50                # Minimum profit threshold

# Market making settings
ENABLE_MARKET_MAKING=true          # Enable market-making simulation
BASE_SPREAD_BPS=30                 # Base spread in basis points (0.3%)
MAX_POSITION_SIZE_ETH=5.0          # Maximum position size

# Volatility settings
VOLATILITY_THRESHOLD=5.0           # Volatility impact threshold (5%)
VOLATILITY_SPREAD_MULTIPLIER=2.0   # Spread multiplier for high volatility

# Trade execution settings (TESTNET ONLY)
ENABLE_TRADE_EXECUTION=false       # Enable trade execution simulation
MAX_GAS_PRICE_GWEI=50              # Maximum gas price
SLIPPAGE_TOLERANCE_BPS=50          # Slippage tolerance (0.5%)
```

## 📊 Output Files

### Arbitrage Opportunities
**Location**: `output/opportunities/arbitrage_YYYY-MM-DD.jsonl`

Contains detailed information about each identified arbitrage opportunity, including validation results, profit calculations, and execution simulations.

### Market-Making Signals
**Location**: `output/market_making/signals_YYYY-MM-DD.jsonl`

Records all generated market-making signals with strategy selection, risk metrics, and volatility assessments.

### Trade Executions
**Location**: `output/executions/trades_YYYY-MM-DD.jsonl`

Logs all simulated trade executions with gas usage, slippage, and profitability metrics.

## 🛡️ Risk Management

### Built-in Safety Features
- **Enhanced Price Validation**: Includes volatility-based sanity checks
- **Volatility Guards**: Prevents execution during extreme market conditions
- **Liquidity Constraints**: Ensures trades don't exceed pool capacity
- **Gas Economics**: Validates profitability after realistic gas costs
- **Circuit Breaker**: Automatic shutdown on consecutive errors
- **Position Limits**: Configurable maximum position sizes with volatility adjustments

## 🚨 Limitations & Disclaimers

### Current Limitations
- **Trade Execution**: Simulation only (testnet-safe)
- **Limited Pools**: Monitors 2 WETH/USD pools on Aerodrome
- **Price Source**: Single CEX reference (Binance)
- **Network**: Base L2 only
- **Volatility**: Historical analysis only (no predictive modeling)

### Important Disclaimers
⚠️ **This bot is for educational and research purposes only**
- No financial advice provided
- Past performance doesn't predict future results
- Trade execution is simulation-only by default
- Always test on testnet before any mainnet use
- Understand the risks of DeFi trading and high volatility
- Volatility analysis is statistical, not predictive

## 🛣️ Roadmap

### Upcoming Features (v0.6.0+)
- [ ] Real Trading Integration: Actual transaction execution with safety limits
- [ ] Multi-DEX Support: Uniswap V4, SushiSwap integration
- [ ] Predictive Volatility: ML-based volatility forecasting
- [ ] Advanced Strategies: Reinforcement learning for strategy selection
- [ ] Portfolio Management: Multi-token inventory management
- [ ] Web Dashboard: Real-time monitoring interface
- [ ] Cross-Chain Arbitrage: Multi-chain opportunity detection

## 🤝 Contributing

We welcome contributions! Please:
1. Fork the repository
2. Create a feature branch
3. Test with Docker: `docker-compose up --build`
4. Add tests if applicable
5. Submit a pull request

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.

## 🙏 Acknowledgments

- **Aerodrome Finance** for providing the DEX infrastructure
- **Binance** for reliable price feeds
- **Alchemy** for robust RPC services
- **Base** for the Layer 2 infrastructure
- **Rust Community** for excellent DeFi tooling

---

**Built with ❤️ and Rust 🦀 | Optimized for Base L2 ⚡**

*For support, questions, or feature requests, please open an issue on GitHub.*