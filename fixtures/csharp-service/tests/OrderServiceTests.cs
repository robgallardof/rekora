namespace Shop.Orders.Tests;

public class OrderServiceTests
{
    public void UpdateLine_ReplacesExistingSku()
    {
        var order = new Order();
        order.ReplaceLine(new OrderLine("A", 1, 10m));
        order.ReplaceLine(new OrderLine("A", 2, 10m));
    }
}
