function alphaToken(): string {
  return "alpha";
}

function betaToken(): string {
  return "beta";
}

function gammaToken(): string {
  return "gamma";
}

const betaAlias = betaToken;
const alphaAlias = alphaToken;
const gammaAlias = gammaToken;

export function entry(): string {
  return gammaAlias() + alphaAlias() + betaAlias();
}

entry();
