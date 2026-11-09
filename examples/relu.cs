public static class MathOperations
{
    [GpuKernel]
    public static float Relu(float value)
    {
        if (value < 0.0f)
        {
            return 0.0f;
        }

        return value;
    }
}
