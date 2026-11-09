public static class MathOperations
{
    [GpuKernel]
    public static float WeightedSum(float a, float b, float weight)
    {
        float weighted = a * weight;
        return weighted + b;
    }
}
