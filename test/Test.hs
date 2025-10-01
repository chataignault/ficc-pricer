{-# LANGUAGE ScopedTypeVariables #-}

module Main (main) where

import Test.QuickCheck
import Data.Map.Strict (Map)
import qualified Data.Map.Strict as Map
import Data.Time
import FX.Algebra.Contract
import FX.Algebra.Observable
import FX.Algebra.Combinators
import FX.Pricing.Core
import FX.Pricing.MarketData

main :: IO ()
main = do
  putStrLn "Testing FX Pricing Algebra Laws..."
  putStrLn "\n=== Monoid Laws ==="

  putStrLn "\n1. Testing left identity: Zero <> c = c"
  quickCheck prop_leftIdentity

  putStrLn "\n2. Testing right identity: c <> Zero = c"
  quickCheck prop_rightIdentity

  putStrLn "\n3. Testing associativity: (a <> b) <> c = a <> (b <> c)"
  quickCheck prop_associativity

  putStrLn "\n4. Testing commutativity: a <> b = b <> a"
  quickCheck prop_commutativity

  putStrLn "\n=== Pricing Laws ==="

  putStrLn "\n5. Testing Zero pricing: price Zero = 0"
  quickCheck prop_zeroPrice

  putStrLn "\n6. Testing Combine additivity: price (a <> b) = price a + price b"
  quickCheck prop_combineAdditive

  putStrLn "\n7. Testing Scale linearity: price (Scale α c) = α * price c"
  quickCheck prop_scaleLinear

  putStrLn "\n8. Testing distributivity: Scale α (a <> b) = Scale α a <> Scale α b"
  quickCheck prop_scaleDistributive

  putStrLn "\nAll tests completed!"

-- | Generate arbitrary contracts for testing
instance Arbitrary Contract where
  arbitrary = sized genContract
    where
      genContract 0 = oneof
        [ pure Zero
        , Spot <$> arbitrary <*> arbitrary
        , ZCB <$> arbitrary <*> arbitrary
        ]
      genContract n = oneof
        [ pure Zero
        , Spot <$> arbitrary <*> arbitrary
        , ZCB <$> arbitrary <*> arbitrary
        , Scale <$> arbitrary <*> genContract (n `div` 2)
        , Combine <$> genContract (n `div` 2) <*> genContract (n `div` 2)
        ]

instance Arbitrary Currency where
  arbitrary = elements [USD, EUR, GBP, JPY, CHF, AUD, CAD]

-- Generate arbitrary dates (within reasonable range)
instance Arbitrary Day where
  arbitrary = ModifiedJulianDay <$> choose (50000, 70000)

-- | Property: Left identity
prop_leftIdentity :: Contract -> Bool
prop_leftIdentity c = (Zero <> c) `eqContract` c

-- | Property: Right identity
prop_rightIdentity :: Contract -> Bool
prop_rightIdentity c = (c <> Zero) `eqContract` c

-- | Property: Associativity
prop_associativity :: Contract -> Contract -> Contract -> Bool
prop_associativity a b c =
  ((a <> b) <> c) `eqContract` (a <> (b <> c))

-- | Property: Commutativity
prop_commutativity :: Contract -> Contract -> Bool
prop_commutativity a b = (a <> b) `eqContract` (b <> a)

-- | Property: Zero pricing
prop_zeroPrice :: Property
prop_zeroPrice =
  let market = testMarket
      params = testParams
  in price Zero market params === 0.0

-- | Property: Combine is additive under pricing
prop_combineAdditive :: Contract -> Contract -> Property
prop_combineAdditive c1 c2 =
  let market = testMarket
      params = testParams
      p1 = price c1 market params
      p2 = price c2 market params
      pCombined = price (Combine c1 c2) market params
  in counterexample
       ("price c1 = " ++ show p1 ++
        ", price c2 = " ++ show p2 ++
        ", price (c1 <> c2) = " ++ show pCombined) $
     abs (pCombined - (p1 + p2)) < 1e-10

-- | Property: Scale is linear under pricing
prop_scaleLinear :: Double -> Contract -> Property
prop_scaleLinear alpha c =
  let market = testMarket
      params = testParams
      p = price c market params
      pScaled = price (Scale alpha c) market params
  in counterexample
       ("price c = " ++ show p ++
        ", price (Scale " ++ show alpha ++ " c) = " ++ show pScaled ++
        ", expected = " ++ show (alpha * p)) $
     abs (pScaled - alpha * p) < 1e-8

-- | Property: Scale distributes over Combine
prop_scaleDistributive :: Double -> Contract -> Contract -> Property
prop_scaleDistributive alpha c1 c2 =
  let market = testMarket
      params = testParams
      lhs = price (Scale alpha (Combine c1 c2)) market params
      rhs = price (Combine (Scale alpha c1) (Scale alpha c2)) market params
  in counterexample
       ("LHS = " ++ show lhs ++ ", RHS = " ++ show rhs) $
     abs (lhs - rhs) < 1e-8

-- | Structural equality for contracts (ignoring observables for simplicity)
-- Handles associativity and commutativity of Combine by comparing multisets
eqContract :: Contract -> Contract -> Bool
eqContract c1 c2 = multisetEq (flatten c1) (flatten c2)
  where
    -- Flatten Combine trees into a list of atomic contracts
    flatten :: Contract -> [Contract]
    flatten Zero = []
    flatten (Scale _ Zero) = []
    flatten (Combine a b) = flatten a ++ flatten b
    flatten c = [c]

    -- Check if two multisets are equal (order-independent comparison)
    multisetEq :: [Contract] -> [Contract] -> Bool
    multisetEq [] [] = True
    multisetEq [] _ = False
    multisetEq _ [] = False
    multisetEq (x:xs) ys = case removeFirst x ys of
      Nothing -> False
      Just ys' -> multisetEq xs ys'

    -- Remove first occurrence of element from list
    removeFirst :: Contract -> [Contract] -> Maybe [Contract]
    removeFirst _ [] = Nothing
    removeFirst x (y:ys)
      | atomicEq x y = Just ys
      | otherwise = (y:) <$> removeFirst x ys

    -- Equality for atomic (non-Combine) contracts
    atomicEq :: Contract -> Contract -> Bool
    atomicEq Zero Zero = True
    atomicEq (Scale _ Zero) Zero = True
    atomicEq Zero (Scale _ Zero) = True
    atomicEq (Spot c1 c2) (Spot c1' c2') = c1 == c1' && c2 == c2'
    atomicEq (Forward d r c1 c2) (Forward d' r' c1' c2') =
      d == d' && r == r' && c1 == c1' && c2 == c2'
    atomicEq (ZCB c d) (ZCB c' d') = c == c' && d == d'
    atomicEq (Scale a c1) (Scale a' c2) = a == a' && eqContract c1 c2
    atomicEq (EurOption ot k d c1 c2) (EurOption ot' k' d' c1' c2') =
      ot == ot' && k == k' && d == d' && c1 == c1' && c2 == c2'
    atomicEq _ _ = False

-- | Test market data
testMarket :: MarketState
testMarket = MarketState
  { spotRates = Map.fromList
      -- Major pairs vs USD
      [ ((EUR, USD), 1.10)
      , ((GBP, USD), 1.25)
      , ((USD, JPY), 110.0)
      , ((USD, CHF), 0.92)
      , ((AUD, USD), 0.74)
      , ((USD, CAD), 1.27)
      -- EUR crosses
      , ((EUR, GBP), 0.88)
      , ((EUR, JPY), 121.0)
      , ((EUR, CHF), 1.01)
      , ((EUR, AUD), 1.49)
      , ((EUR, CAD), 1.40)
      -- GBP crosses
      , ((GBP, JPY), 137.5)
      , ((GBP, CHF), 1.15)
      , ((GBP, AUD), 1.69)
      , ((GBP, CAD), 1.59)
      -- Other crosses
      , ((CHF, JPY), 119.6)
      , ((AUD, JPY), 81.5)
      , ((CAD, JPY), 86.6)
      , ((AUD, CHF), 0.80)
      , ((AUD, CAD), 0.94)
      , ((CHF, CAD), 1.38)
      ]
  , discountCurves = Map.fromList
      [ (USD, \_ -> 0.98)
      , (EUR, \_ -> 0.99)
      , (GBP, \_ -> 0.97)
      , (JPY, \_ -> 1.00)
      , (CHF, \_ -> 0.99)
      , (AUD, \_ -> 0.96)
      , (CAD, \_ -> 0.97)
      ]
  , volSurfaces = Map.fromList
      [ ((EUR, USD), \_ _ -> 0.10)
      , ((GBP, USD), \_ _ -> 0.12)
      , ((USD, JPY), \_ _ -> 0.08)
      ]
  , correlations = Map.empty
  }

-- | Test pricing parameters
testParams :: PricingParams
testParams = PricingParams
  { valuationDate = fromGregorian 2025 1 1
  , numeraire = USD
  , model = BlackScholes
  }